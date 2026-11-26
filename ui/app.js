"use strict";
const invoke = window.__TAURI__?.core?.invoke;
const $ = id => document.getElementById(id);
const esc = value => String(value??"").replace(/[&<>"']/g,c=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
const fmt = value => Number.isFinite(value)?value.toFixed(3)+"s":"—";
const clock = value => {
 if(!Number.isFinite(value))return "—";
 const ms=Math.max(0,Math.floor(value*1000));
 return String(Math.floor(ms/60000)).padStart(2,"0")+":"+String(Math.floor(ms/1000)%60).padStart(2,"0")+"."+String(ms%1000).padStart(3,"0");
};
const date = value => value?new Date(value).toLocaleString("zh-CN",{hour12:false}):"旧版记录 · 日期未保存";
const keys=["run","impact_1","impact_2","impact_3","settlement"];
const names=["跑图","第一次撞击","第二次撞击","第三次撞击","结算时间"];
const colors=["#69adff","#f8b55c","#57cfd8","#d68cf0","#81caa3"];
const icons={
 home:'<path d="m3 10 9-7 9 7v11H3z"/><path d="M9 21v-8h6v8"/>',
 box:'<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M9 14h6"/>',
 chart:'<path d="M4 20V11m6 9V6m6 14v-9m5 9V3"/>',
 settings:'<path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"/><path d="M19 13v-2l2-1-2-4-2 1-2-2V3H9v2L7 7 5 6l-2 4 2 1v2l-2 1 2 4 2-1 2 2v2h6v-2l2-2 2 1 2-4z"/>',
 clock:'<circle cx="12" cy="12" r="9"/><path d="M12 7v5l4 2"/>',
 star:'<path d="m12 2 3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1z"/>',
 edit:'<path d="M4 20h5l11-11-5-5L4 15zM13 6l5 5"/>',
 trash:'<path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7"/>',
 left:'<path d="m15 5-7 7 7 7"/>',
 right:'<path d="m9 5 7 7-7 7"/>',
 refresh:'<path d="M20 7v5h-5M4 17v-5h5M4 12a8 8 0 0 1 14-5l2 5M20 12a8 8 0 0 1-14 5l-2-5"/>',
 sun:'<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l2 2m10 10 2 2M5 19l2-2M17 7l2-2"/>',
 moon:'<path d="M20 15A9 9 0 0 1 9 4 9 9 0 1 0 20 15z"/>',
 copy:'<rect x="8" y="8" width="12" height="13" rx="1.5"/><path d="M16 8V3H3v13h5"/>',
 check:'<path d="m4 12 5 5L20 6"/>',
 menu:'<path d="M4 6h16M4 12h16M4 18h16"/>'
};
const svg = name => '<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">'+(icons[name]||icons.clock)+'</svg>';
const btn=(name,label,action,extra="")=>'<button type="button" title="'+esc(label)+'" aria-label="'+esc(label)+'" data-action="'+action+'" '+extra+'>'+svg(name)+'</button>';
const pages=[["home","主页","home"],["storage","历史记录","box"],["analytics","统计分析","chart"],["settings","设置","settings"]];
const state={
 page:"home",runs:[],selected:null,revision:-1,status:{mode:"waiting"},overlay:null,
 sort:"newest",filter:"complete",query:"",pageNo:0,
 light:localStorage.getItem("ropa-theme")==="light",busy:false,
};
let toastTimeout, pendingId=null, deleteId=null, polling=false;
function toast(message) {
 $("toast").textContent=String(message);$("toast").classList.add("show");
 clearTimeout(toastTimeout);toastTimeout=setTimeout(()=>$("toast").classList.remove("show"),3400);
}
function completed(record) {
 return record?.status==="Succeeded"
 && Number.isFinite(record.duration_seconds) && record.duration_seconds>0
 && record.splits?.length===5
 && record.splits.every((split,index)=>split.key===keys[index]);
}
function completedRecords(){return state.runs.filter(completed)}
function selected() {
 return state.runs.find(record=>record.id===state.selected)
 || state.runs[0] || null;
}
function best(){return completedRecords().reduce((acc,r)=>!acc||r.duration_seconds<acc.duration_seconds?r:acc,null)}
function average(values){return values.length?values.reduce((a,b)=>a+b,0)/values.length:null}
function averages(){
 return keys.map(key=>average(completedRecords().map(r=>r.splits.find(s=>s.key===key)?.split_seconds).filter(Number.isFinite)));
}
function badge(record){
 const status=({Succeeded:"正常完成",Failed:"任务失败",Aborted:"已中止",InProgress:"进行中"})[record.status]||record.status;
 const style=record.status==="Succeeded"?"success":record.status==="Failed"?"failed":"aborted";
 return '<span class="status-chip '+style+'">'+esc(status)+'</span>';
}
function delta(value,baseline){
 if(!Number.isFinite(value)||!Number.isFinite(baseline))return "";
 const diff=value-baseline;
 return '<span class="'+(diff<0?"fast":diff>0?"slow":"muted")+'">'+(diff>0?"+":"")+diff.toFixed(3)+"s</span>";
}
function phaseGrid(record){
 const avg=averages();
 return '<div class="phase-grid">'+names.map((name,index)=>{
  const split=record?.splits.find(s=>s.key===keys[index]);
  return '<article class="phase-card"><div class="phase-title"><span>'+name+'</span><span style="color:'+colors[index]+'">0'+(index+1)+'</span></div>'+
   '<strong>'+clock(split?.cumulative_seconds)+'</strong>'+
   '<div class="phase-sub"><span>阶段用时：'+fmt(split?.split_seconds)+'</span><span>'+delta(split?.split_seconds,avg[index])+'</span></div></article>';
 }).join("")+'</div>';
}
function metrics(record){
 const rows=[["总用时",record?.duration_seconds,"clock","#69adff"],...names.map((name,index)=>[name,record?.splits.find(s=>s.key===keys[index])?.split_seconds,"clock",colors[index]])];
 const pb=best();
 return '<div class="metric-grid">'+rows.map(([name,value,k,color],index)=>
 '<article class="metric"><div class="metric-top"><span class="metric-icon" style="background:'+color+'">'+svg(k)+'</span><span>'+name+'</span></div>'+
 '<div class="metric-value '+(index===0&&completed(record)&&pb?.id===record?.id?"pb-value":"")+'">'+(Number.isFinite(value)?value.toFixed(3):"—")+(Number.isFinite(value)?"<small>s</small>":"")+'</div></article>'
 ).join("")+'</div>';
}
function renderHome(){
 const record=selected();
 const live=state.status.active;
 const active=live?{splits:state.status.splits,duration_seconds:state.status.elapsed_seconds}:record;
 const list=state.runs;
 const index=record?list.findIndex(r=>r.id===record.id):-1;
 const summary=live?'<div class="section-head"><h2>正在进行的任务</h2><span class="status-chip success">● 实时计时 · '+clock(state.status.elapsed_seconds)+'</span></div>'
 :'<div class="section-head"><h2>'+(record?esc(record.name):"等待第一场任务")+'</h2>'+(record?badge(record):"")+'</div>';
 $("content").innerHTML=summary+metrics(active)+
 '<div class="section-head"><h2>五阶段竞速数据</h2><span class="muted">累计时间 / 阶段耗时 / 较平均</span></div>'+
 phaseGrid(active)+
 (record?'<div class="detail-panel" style="margin-top:14px"><div class="section-head"><h2>已保存的任务</h2><span class="muted">'+esc(date(record.started_utc))+'</span></div>'+
 '<div class="run-title">'+badge(record)+'<span class="name">'+esc(record.name)+'</span>'+btn("edit","重命名","rename:"+record.id)+btn("star",record.favorite?"取消收藏":"收藏","favorite:"+record.id)+(completed(record)?'<span class="muted"> · '+clock(record.duration_seconds)+'</span>':"")+'</div></div>':
 '<div class="empty-box">启动 Warframe 并进入蝠力使任务，程序会自动保存竞速记录。</div>');
 $("toolbar").innerHTML=btn("left","上一场（较旧）","older",index>=list.length-1?"disabled":"")+
 btn("right","下一场（较新）","newer",index<=0?"disabled":"")+
 btn(state.light?"moon":"sun","切换主题","theme")+btn("settings","设置","page:settings");
}
function filtered(){
 let records=state.runs.filter(r=>{
  if(state.filter==="complete"&&!completed(r))return false;
  if(state.filter==="incomplete"&&completed(r))return false;
  if(state.filter==="favorite"&&!r.favorite)return false;
  const query=state.query.trim().toLowerCase();
  return !query || (r.name||"").toLowerCase().includes(query)||String(r.id).includes(query);
 });
 records.sort((a,b)=>state.sort==="fastest"
  ? (a.duration_seconds??Infinity)-(b.duration_seconds??Infinity)
  : state.sort==="oldest"?a.id-b.id:b.id-a.id);
 return records;
}
function renderStorage(){
 const records=filtered();const per=14;const pages=Math.max(1,Math.ceil(records.length/per));
 state.pageNo=Math.min(state.pageNo,pages-1);
 const current=records.slice(state.pageNo*per,(state.pageNo+1)*per);
 $("toolbar").innerHTML=btn("refresh","刷新","refresh")+btn(state.light?"moon":"sun","切换主题","theme");
 $("content").innerHTML='<div class="section-head"><h2>历史记录 <span class="muted" style="font-size:14px">'+state.runs.length+' 场</span></h2><span class="muted">本地 SQLite · 自动去重</span></div>'+
 '<div class="ropa-toolbar"><input id="search" placeholder="搜索记录名称 / 编号..." value="'+esc(state.query)+'" aria-label="搜索历史">'+
 '<select id="filter" aria-label="筛选状态"><option value="complete">正常完成</option><option value="all">全部记录</option><option value="incomplete">未完成 / 中断</option><option value="favorite">已收藏</option></select>'+
 '<select id="sort" aria-label="排序"><option value="newest">最近优先</option><option value="oldest">最早优先</option><option value="fastest">用时最短</option></select></div>'+
 (current.length?'<div class="ropa-list">'+current.map(r=>
 '<article class="ropa-run"><div class="run-info"><strong>'+esc(r.name)+'</strong>'+
 '<div>'+badge(r)+' <small>'+esc(date(r.started_utc))+'</small>'+(r.favorite?' <span class="pb-value">★ 已收藏</span>':"")+'</div></div>'+
 '<div class="run-duration">'+clock(r.duration_seconds)+'</div>'+
 '<div class="run-actions">'+btn("home","查看记录","open:"+r.id)+btn("edit","重命名","rename:"+r.id)+
 btn("star",r.favorite?"取消收藏":"收藏","favorite:"+r.id)+btn("copy","复制成绩","copy:"+r.id)+
 btn("trash","删除记录","delete:"+r.id,'class="danger"')+'</div></article>'
 ).join("")+'</div>':'<div class="empty-box">没有符合条件的记录。</div>')+
 '<div class="ropa-toolbar" style="justify-content:flex-end;margin-top:14px">'+btn("left","上一页","prev",state.pageNo===0?"disabled":"")+
 '<span class="muted">第 '+(state.pageNo+1)+' / '+pages+' 页</span>'+btn("right","下一页","next",state.pageNo>=pages-1?"disabled":"")+'</div>';
 $("filter").value=state.filter;$("sort").value=state.sort;
}
function renderAnalytics(){
 const valid=completedRecords();
 const values=valid.map(r=>r.duration_seconds);
 const pb=best();const mean=average(values);
 const phaseAvg=averages();
 const fastest=[...valid].sort((a,b)=>a.duration_seconds-b.duration_seconds).slice(0,12);
 $("toolbar").innerHTML=btn("refresh","刷新","refresh")+btn(state.light?"moon":"sun","切换主题","theme");
 $("content").innerHTML='<div class="section-head"><h2>统计分析</h2><span class="muted">只统计日志证据完整的成功任务</span></div>'+
 '<div class="stat-grid"><article class="stat-card"><span class="muted">有效成绩</span><strong>'+valid.length+'</strong></article>'+
 '<article class="stat-card"><span class="muted">个人最佳 · PB</span><strong class="pb-value">'+clock(pb?.duration_seconds)+'</strong></article>'+
 '<article class="stat-card"><span class="muted">平均用时</span><strong>'+clock(mean)+'</strong></article></div>'+
 '<div class="bar-panel"><h2>平均阶段用时</h2>'+
 names.map((name,index)=>'<div class="bar-row"><span>'+name+'</span><div class="bar-track"><div class="bar-fill" style="width:'+
 (Number.isFinite(phaseAvg[index])?Math.max(2,phaseAvg[index]/Math.max(...phaseAvg.filter(Number.isFinite),1)*100):0)+'%;background:'+colors[index]+'"></div></div><strong>'+fmt(phaseAvg[index])+'</strong></div>').join("")+'</div>'+
 '<div class="bar-panel"><h2>最佳成绩排行</h2>'+
 (fastest.length?fastest.map((r,i)=>'<div class="bar-row"><span>#'+(i+1)+'</span><div class="bar-track"><div class="bar-fill" style="width:'+
 (r.duration_seconds?Math.max(2,(fastest[0].duration_seconds/r.duration_seconds)*100):0)+'%"></div></div><button data-action="open:'+r.id+'" title="查看记录">'+clock(r.duration_seconds)+'</button></div>').join(""):'<p class="muted">暂无完整的蝠力使竞速成绩。</p>')+'</div>';
}
function settingRow(title,description,control){
 return '<div class="settings-row"><div><strong>'+title+'</strong><p>'+description+'</p></div>'+control+'</div>';
}
function renderSettings(){
 $("toolbar").innerHTML=btn(state.light?"moon":"sun","切换主题","theme");
 const o=state.overlay;
 if(!o){$("content").innerHTML='<div class="empty-box">正在加载设置…</div>';return;}
 const c=o.config;
 const cb=(key,checked)=>'<input aria-label="'+key+'" type="checkbox" data-setting="'+key+'" '+(checked?"checked":"")+'>';
 const slider=(key,value,min,max,suffix)=>'<span class="range-wrap"><input aria-label="'+key+'" data-setting="'+key+'" type="range" min="'+min+'" max="'+max+'" value="'+value+'"><strong>'+value+suffix+'</strong></span>';
 $("content").innerHTML=
 '<div class="section-head"><h2>应用设置</h2><span class="muted">所有设置存储在本地</span></div>'+
 '<div class="settings-panel"><h2>日志监控</h2>'+
 settingRow("Warframe EE.log","默认自动定位；也可以自定义日志的绝对路径",'<input id="log-path" style="min-width:320px" value="'+esc(state.status.path)+'">')+
 '<div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px"><button data-action="save-path">保存日志路径</button><button data-action="reset-path">恢复默认路径</button></div></div>'+
 '<div class="settings-panel"><h2>游戏内 Overlay</h2>'+
 settingRow("启用 Overlay","任务开始时显示透明计时卡片",cb("enabled",c.enabled))+
 settingRow("仅在 Warframe 前台显示","切换到其他程序时自动隐藏",cb("foreground_only",c.foreground_only))+
 settingRow("无活动任务时自动隐藏","完成任务后保留最终成绩数秒",cb("auto_hide",c.auto_hide))+
 settingRow("不透明度","调节背景透明程度",slider("opacity",c.opacity,20,100,"%"))+
 settingRow("整体缩放","可在独立调整模式中手动拖动尺寸",slider("scale",c.scale,50,200,"%"))+
 '<div style="display:flex;gap:8px;margin-top:15px"><button data-action="edit-overlay">'+(o.editing?"完成调整":"调整位置与大小")+'</button><button data-action="reset-overlay">恢复默认布局</button></div>'+
 (o.error?'<p class="warn">'+esc(o.error)+'</p>':"")+'</div>'+
 '<div class="settings-panel"><h2>数据存储</h2><p class="muted">继续使用旧版成绩库：%LOCALAPPDATA%\\Warframe\\ropalolyst-analytics.sqlite3。旧成绩自动兼容，删除记录保留去重标记。</p></div>';
}
function render(){
 document.documentElement.classList.toggle("light",state.light);
 $("rail").innerHTML=pages.map(([id,label,k])=>btn(k,label,"page:"+id,'class="'+(state.page===id?"active":"")+'"')).join("");
 $("subtitle").textContent=({home:"你的蝠力使竞速记录",storage:"管理所有历史任务",analytics:"竞速数据与成绩趋势",settings:"个性化与监控设置"})[state.page];
 ({home:renderHome,storage:renderStorage,analytics:renderAnalytics,settings:renderSettings})[state.page]();
 updateStatus();
}
function updateStatus(){
 const status=state.status;
 const labels={waiting:"等待 Warframe 日志",retry:"日志读取异常 · 正在重试",live:"日志监听正常",offline:"无法连接分析核心"};
 const el=$("status");
 el.innerHTML='<i class="'+esc(status.mode||"waiting")+'"></i><span>'+esc(labels[status.mode]||status.mode)+(status.active?" · 正在计时":"")+'</span>';
 el.title=[status.path,status.error].filter(Boolean).join("\n");
 $("progress").textContent=status.active?"任务 "+clock(status.elapsed_seconds):"";
}
async function refresh(force=false){
 if(!invoke||polling)return;polling=true;
 try{
  const previousActive=state.status.active;
  state.status=await invoke("get_status");
  const changed=force||state.status.revision!==state.revision;
  if(changed){
   const follow=state.selected===null||state.selected===state.runs[0]?.id;
   state.runs=await invoke("get_runs");
   if(follow||!state.runs.some(r=>r.id===state.selected))state.selected=state.runs[0]?.id??null;
   state.revision=state.status.revision;
  }
  if(changed||(state.page==="home"&&(state.status.active||previousActive)))render();
  else updateStatus();
 }catch(error){
  state.status={mode:"offline",error:String(error)};
  updateStatus();
 }finally{polling=false;}
}
function report(record){
 return ['蝠力使竞速分析','任务：'+record.name,'状态：'+record.status,'用时：'+clock(record.duration_seconds),
  ...record.splits.map(s=>s.label+'：'+clock(s.cumulative_seconds)+'（阶段 '+fmt(s.split_seconds)+'）')].join("\n");
}
async function action(name){
 const [kind,arg]=name.split(":");
 if(kind==="page"){
  state.page=arg;
  if(arg==="settings") {
   try{state.overlay=await invoke("get_overlay_settings");}catch(e){toast(e)}
  }
  render();return;
 }
 if(kind==="theme"){state.light=!state.light;localStorage.setItem("ropa-theme",state.light?"light":"dark");render();return;}
 if(kind==="refresh"){await refresh(true);return;}
 if(kind==="older"||kind==="newer"){
  const i=state.runs.findIndex(r=>r.id===state.selected);
  state.selected=state.runs[Math.max(0,Math.min(state.runs.length-1,i+(kind==="older"?1:-1)))]?.id??null;render();return;
 }
 if(kind==="open"){state.selected=Number(arg);state.page="home";render();return;}
 if(kind==="next"||kind==="prev"){state.pageNo+=kind==="next"?1:-1;render();return;}
 if(kind==="rename"){
  pendingId=Number(arg);const r=state.runs.find(r=>r.id===pendingId);$("dialog-input").value=r?.name??"";$("edit").showModal();$("dialog-input").focus();return;
 }
 if(kind==="delete"){
  deleteId=Number(arg);$("confirm-delete").showModal();return;
 }
 if(kind==="favorite"){
  await invoke("favorite_run",{id:Number(arg)});await refresh(true);return;
 }
 if(kind==="copy"){
  const record=state.runs.find(r=>r.id===Number(arg));
  if(record){await navigator.clipboard.writeText(report(record));toast("已复制本次成绩");}return;
 }
 if(kind==="save-path"||kind==="reset-path"){
  const path=kind==="reset-path"?"":$("log-path").value;
  await invoke("set_log_path",{path});toast("日志路径已保存");await refresh(true);return;
 }
 if(kind==="edit-overlay"){
  state.overlay=await invoke("edit_overlay",{editing:!state.overlay?.editing});render();return;
 }
 if(kind==="reset-overlay"){
  state.overlay=await invoke("reset_overlay");render();return;
 }
}
document.addEventListener("click",async event=>{
 const target=event.target.closest("[data-action]");
 if(!target)return;
 try{await action(target.dataset.action);}catch(error){toast(String(error))}
});
document.addEventListener("change",async event=>{
 if(event.target.id==="filter"){state.filter=event.target.value;state.pageNo=0;render();return;}
 if(event.target.id==="sort"){state.sort=event.target.value;state.pageNo=0;render();return;}
 const key=event.target.dataset.setting;
 if(key&&state.overlay){
  const c={...state.overlay.config};
  c[key]=event.target.type==="checkbox"?event.target.checked:Number(event.target.value);
  try{state.overlay=await invoke("set_overlay_settings",{config:c});render();}
  catch(error){toast(String(error));render();}
 }
});
document.addEventListener("input",event=>{
 if(event.target.id==="search"){
  const selection=event.target.selectionStart;
  state.query=event.target.value;state.pageNo=0;render();
  const input=$("search");input.focus();input.setSelectionRange(selection,selection);
 }
});
$("edit-form").addEventListener("submit",async event=>{
 event.preventDefault();
 try{
  await invoke("rename_run",{id:pendingId,name:$("dialog-input").value.trim()});
  $("edit").close();await refresh(true);toast("已保存记录名称");
 }catch(error){toast(String(error))}
});
$("edit-cancel").addEventListener("click",()=>$("edit").close());
$("confirm-delete").addEventListener("close",async()=>{
 if($("confirm-delete").returnValue!=="delete")return;
 try{await invoke("delete_run",{id:deleteId});await refresh(true);toast("记录已删除");}
 catch(error){toast(String(error))}
});
if(!invoke){
 state.status={mode:"offline",error:"请使用桌面版程序打开"};
 render();
}else{
 refresh(true);
 setInterval(()=>refresh(false),300);
}
