import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {Snapshot,Profile,Connection,Settings,Answer} from "./types";
import {defaultRules} from "./types";
export const desktop="__TAURI_INTERNALS__" in window;
export const preview=!desktop && new URLSearchParams(location.search).get("preview")==="1";
export async function rpc<T=unknown>(action:string,payload:Record<string,unknown>={}):Promise<T>{
  if(desktop)return invoke<T>("rpc",{action,payload});
  if(preview)return previewRpc(action,payload) as T;
  throw new Error("请在桌面程序中使用；浏览器界面预览请添加 ?preview=1");
}
export async function subscribe<T>(event:string,handler:(v:T)=>void):Promise<()=>void>{
  if(desktop)return listen<T>(event,e=>handler(e.payload));
  const listener=(e:Event)=>handler((e as CustomEvent).detail);
  window.addEventListener(event,listener);return()=>window.removeEventListener(event,listener);
}
export async function pickFile(extensions:string[]):Promise<string|null>{
  if(!desktop)throw new Error("文件选择需要桌面程序");
  const path=await open({multiple:false,filters:[{name:"支持的文件",extensions}]});
  return typeof path==="string"?path:null;
}
export async function saveFile(defaultPath:string,extensions:string[]):Promise<string|null>{
  if(!desktop)throw new Error("文件导出需要桌面程序");
  return save({defaultPath,filters:[{name:"文件",extensions}]});
}
const demo:Snapshot={
  profiles:[],connections:[],has_keys:{},
  settings:{active_profile:null,preparation_connection:null,answer_connections:[],race:false,timeout_seconds:60,region:null,overlay:{x:100,y:80,width:1100,height:430,font_size:28,font_family:"Segoe UI"},capture_shortcut:"Ctrl+Alt+Space",toggle_shortcut:"Ctrl+Alt+H"},
  caption:{request_id:"",profile_id:"",revision:"",question:"",answer:"",status:"",detail:"",elapsed_ms:0,cached:false},
  ocr_status:"浏览器预览不运行屏幕 OCR",ocr_ready:false,windows:false,visible:true,jobs:[]
};
const demoAnswers:Answer[]=[];
function previewRpc(action:string,p:Record<string,unknown>):unknown{
  switch(action){
    case"snapshot":return structuredClone(demo);
    case"create_profile":{const profile:Profile={id:crypto.randomUUID(),name:String(p.name),goal:"",rules:{...defaultRules},sources:[],clarification:"",draft:null,active:null};demo.profiles.push(profile);return structuredClone(profile);}
    case"save_profile":{const v=p.profile as Profile;demo.profiles=demo.profiles.map(x=>x.id===v.id?structuredClone(v):x);return v;}
    case"activate_profile":{const v=demo.profiles.find(x=>x.id===p.id)!;if(!v.draft?.identity)throw Error("请填写个人定位");v.active={revision:"preview",compiled:v.draft,rules:v.rules,goal:v.goal};demo.settings.active_profile=v.id;return v;}
    case"delete_profile":demo.profiles=demo.profiles.filter(x=>x.id!==p.id);return null;
    case"save_settings":demo.settings=structuredClone(p.settings as Settings);return demo.settings;
    case"save_connection":{const c=p.connection as Connection;demo.connections=demo.connections.filter(x=>x.id!==c.id).concat(c);demo.has_keys[c.id]=!!p.secret;return c;}
    case"delete_connection":demo.connections=demo.connections.filter(x=>x.id!==p.id);return null;
    case"answers":return demoAnswers.filter(x=>x.profile_id===p.id);
    case"toggle":demo.visible=!demo.visible;return demo.visible;
    case"caption":return{caption:demo.caption,overlay:demo.settings.overlay,adjusting:false};
    case"cancel_job":return null;
    default:throw Error("这是界面预览。OCR、模型调用和文件处理需要在桌面程序中运行。");
  }
}
