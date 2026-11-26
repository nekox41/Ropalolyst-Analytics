"use strict";
const invoke = window.__TAURI__?.core?.invoke;
const $ = id => document.getElementById(id);
const format = seconds => {
  const ms = Math.max(0,Math.floor((Number(seconds)||0)*1000));
  return String(Math.floor(ms/60000)).padStart(2,"0")+":"+String(Math.floor(ms/1000)%60).padStart(2,"0")+"."+String(ms%1000).padStart(3,"0");
};
let frame;
async function tick() {
  if (!invoke) return;
  try {
    frame = await invoke("get_overlay_frame");
    const editing = frame.settings.editing;
    document.body.classList.toggle("editing",editing);
    document.body.classList.toggle("finished",frame.finished);
    document.documentElement.style.setProperty("--opacity",(frame.settings.config.opacity/100).toFixed(2));
    $("elapsed").textContent=frame.elapsed_seconds == null ? (editing?"02:28.906":"00:00.000") : format(frame.elapsed_seconds);
    $("label").textContent=editing?"计时预览":frame.finished?"最终用时":"任务用时";
    $("state").textContent=editing?"拖动调整":"";
    $("finish").hidden=!editing;
    $("resize").hidden=!editing;
  } catch(error) {
    $("state").textContent="连接失败";
  }
}
$("finish").addEventListener("click",async e=>{e.stopPropagation();try{await invoke("edit_overlay",{editing:false});}catch(e){$("state").textContent=String(e)}});
$("handle").addEventListener("pointerdown",e=>{
  if(!frame?.settings?.editing||e.target.closest("button"))return;
  invoke("drag_overlay",{resize:false}).catch(()=>{});
});
$("resize").addEventListener("pointerdown",e=>{
  e.stopPropagation();
  if(frame?.settings?.editing)invoke("drag_overlay",{resize:true}).catch(()=>{});
});
setInterval(tick,75);
tick();
