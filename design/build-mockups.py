#!/usr/bin/env python3
"""Generate styled mockups: 7 visual styles x 3 scenes -> project/styles/<Style>-<Scene>.dc.html"""
import json, os, pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "project" / "styles"
OUT.mkdir(parents=True, exist_ok=True)

# ---------------------------------------------------------------- shared CSS
BASE_CSS = """
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--text);font-family:var(--body);font-size:14px}
a{color:var(--text)}a:hover{color:var(--accent)}
.app{width:100%;min-height:820px;display:flex;flex-direction:column;position:relative;overflow:hidden;isolation:isolate;background:var(--bg);border:var(--frame-border);border-radius:var(--frame-radius)}
.titlebar{height:38px;display:flex;align-items:center;justify-content:space-between;padding:0 0 0 14px;background:var(--title-bg);color:var(--title-text);border-bottom:var(--hair);font-size:13px}
.titlebar .dot{width:14px;height:14px;border-radius:var(--dot-radius);background:var(--accent)}
.titlebar .win{display:flex}
.titlebar .win span{width:46px;height:38px;display:flex;align-items:center;justify-content:center;border-left:var(--hair)}
.toolbar{display:flex;flex-wrap:wrap;align-items:center;gap:10px;padding:12px 16px;border-bottom:var(--hair);background:var(--surface-2)}
.btn{display:inline-flex;align-items:center;gap:8px;height:40px;padding:0 16px;border:var(--btn-border);border-radius:var(--radius);background:var(--btn-bg);color:var(--btn-text);font-family:var(--body);font-size:14px;font-weight:600;cursor:pointer;white-space:nowrap;box-shadow:var(--btn-shadow);text-decoration:none}
.btn.primary{background:var(--accent-bg);color:var(--accent-text);border:var(--accent-border);box-shadow:var(--accent-shadow)}
.btn.sm{height:32px;padding:0 12px;font-size:13px}
.btn.icon{width:40px;padding:0;justify-content:center}
.btn.icon.sm{width:32px}
.btn .sub{font-size:12px;opacity:.75;border-left:1px solid currentColor;padding-left:8px;font-weight:400}
.hint{font-size:12px;color:var(--muted);line-height:1.45}
.body{display:flex;flex-wrap:wrap;flex:1 1 auto;min-height:0}
.gallery{flex:999 1 560px;min-width:0;padding:0 18px 18px;display:flex;flex-direction:column;gap:16px}
.tabs{display:flex;align-items:center;justify-content:space-between;gap:12px;border-bottom:var(--hair)}
.tab{height:42px;padding:0 16px;border:0;border-bottom:3px solid transparent;background:none;font-family:var(--display);font-size:15px;color:var(--muted);cursor:pointer}
.tab.on{color:var(--text);border-bottom-color:var(--accent)}
.grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:16px}
.card{border:var(--card-border);border-radius:var(--radius-lg);padding:10px;display:flex;flex-direction:column;gap:8px;background:var(--surface);box-shadow:var(--card-shadow);min-width:0;position:relative}
.thumb{height:140px;border-radius:var(--radius);position:relative;overflow:hidden;background:var(--thumb)}
.thumb .tag{position:absolute;left:8px;bottom:6px;font-size:11px;color:var(--tag-text);background:var(--tag-bg);padding:2px 7px;border-radius:var(--radius-sm)}
.name{height:34px;border:var(--input-border);border-radius:var(--radius);padding:0 10px;font-family:var(--mono,var(--body));font-size:13px;width:100%;color:var(--text);background:var(--input-bg)}
.row{display:flex;align-items:center;justify-content:space-between;gap:10px}
.chip{font-size:11px;font-weight:600;padding:3px 9px;border:var(--chip-border);border-radius:var(--radius-pill);color:var(--chip-text);background:var(--chip-bg);white-space:nowrap}
.chip.saved{background:var(--accent-bg);color:var(--accent-text);border:var(--accent-border)}
.day{display:flex;align-items:center;gap:10px;margin-top:4px}
.day h3{margin:0;font-family:var(--display);font-size:14px;font-weight:600;color:var(--text)}
.day span{font-size:12px;color:var(--muted)}
.day::after{content:"";flex:1;height:var(--hair-w);background:var(--hair-c)}
.settings{flex:0 1 300px;min-width:0;border-left:var(--hair);padding:18px;display:flex;flex-direction:column;gap:22px;background:var(--surface-2)}
.group{display:flex;flex-direction:column;gap:11px}
.label{font-family:var(--display);font-size:12px;color:var(--muted);text-transform:var(--label-case);letter-spacing:var(--label-track)}
.toggle{width:44px;height:24px;border:var(--toggle-border);border-radius:var(--radius-pill);position:relative;flex:none;background:var(--toggle-off);padding:0;cursor:pointer}
.toggle::after{content:"";position:absolute;top:3px;left:3px;width:14px;height:14px;border-radius:var(--radius-pill);background:var(--toggle-knob)}
.toggle.on{background:var(--accent-bg)}.toggle.on::after{left:23px;background:var(--accent-text)}
.field{height:36px;border:var(--input-border);border-radius:var(--radius);padding:0 10px;display:flex;align-items:center;font-size:13px;overflow:hidden;white-space:nowrap;background:var(--input-bg);min-width:0;font-family:var(--mono,var(--body))}
.seg{display:flex;border:var(--input-border);border-radius:var(--radius);overflow:hidden;background:var(--input-bg)}
.seg button{flex:1;height:32px;border:0;background:transparent;font-family:var(--body);font-size:13px;color:var(--text);cursor:pointer}
.seg button.on{background:var(--accent-bg);color:var(--accent-text)}
.bottom{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:12px;padding:12px 18px;border-top:var(--hair);background:var(--surface-2)}
.toast{position:absolute;left:50%;transform:translateX(-50%);bottom:84px;display:flex;align-items:center;gap:8px;height:40px;padding:0 16px;border-radius:var(--radius);background:var(--toast-bg);color:var(--toast-text);border:var(--toast-border);font-size:14px;font-weight:600;box-shadow:var(--pop-shadow);white-space:nowrap}
h2.sec{margin:0;font-family:var(--display);font-size:17px;font-weight:600}
.app::before,.app::after{z-index:-1}
/* accessibility: visible focus for every control, demoed on one button per board */
:focus-visible,.kbd-focus{outline:3px solid var(--focus,var(--accent));outline-offset:3px;box-shadow:var(--btn-shadow),0 0 0 6px var(--focus-halo,transparent)}
.card .row{flex-wrap:wrap}
.card .row .hint{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0;flex:1 1 0;text-align:right}
.card .row>div{display:flex;gap:6px;flex-wrap:wrap;justify-content:flex-end}
.srlabel{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}
/* editor */
.canvas{position:relative;flex:1 1 auto;min-height:690px;background:var(--canvas)}
.pill{display:flex;align-items:center;gap:2px;padding:4px;border:var(--pop-border);border-radius:var(--radius-lg);background:var(--surface);box-shadow:var(--pop-shadow)}
.tool{width:40px;height:40px;display:flex;align-items:center;justify-content:center;border:0;border-radius:var(--radius);background:transparent;color:var(--text);cursor:pointer;position:relative;padding:0}
.tool.on{background:var(--accent-bg);color:var(--accent-text)}
.tool .k{position:absolute;right:4px;bottom:2px;font-size:10px;opacity:.8}
.sep{width:1px;height:28px;background:var(--hair-c);margin:0 4px}
.panel{position:absolute;left:16px;top:84px;width:200px;padding:14px;border:var(--pop-border);border-radius:var(--radius-lg);background:var(--surface);box-shadow:var(--pop-shadow);display:flex;flex-direction:column;gap:14px}
.sw{width:24px;height:24px;border-radius:var(--radius-sm);border:var(--input-border);padding:0;cursor:pointer}
.sw.on{outline:2px solid var(--accent);outline-offset:2px}
.opt{height:28px;min-width:28px;padding:0 8px;border:var(--input-border);border-radius:var(--radius-sm);background:var(--input-bg);color:var(--text);font-family:var(--body);font-size:12px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;gap:4px}
.opt.on{background:var(--accent-bg);color:var(--accent-text);border:var(--accent-border)}
.corner{position:absolute;display:flex;align-items:center;gap:2px;padding:4px;border:var(--pop-border);border-radius:var(--radius);background:var(--surface);box-shadow:var(--pop-shadow)}
.corner button{height:32px;min-width:32px;padding:0 8px;border:0;background:transparent;color:var(--text);font-family:var(--body);font-size:13px;cursor:pointer;border-radius:var(--radius-sm);display:inline-flex;align-items:center;justify-content:center}
.shot{position:absolute;left:260px;top:70px;width:760px;height:475px;background:var(--shot-bg);border:var(--card-border);box-shadow:var(--shot-shadow);border-radius:var(--radius-sm)}
.shot .ui{position:absolute;border:1.5px solid var(--shot-line);border-radius:var(--radius-sm);background:var(--shot-fill)}
.anno{position:absolute}
.handle{position:absolute;width:10px;height:10px;border:2px solid var(--anno);background:var(--surface);border-radius:2px}
.track{height:4px;background:var(--hair-c);border-radius:2px;position:relative;margin:8px 6px}
.track i{position:absolute;left:0;top:0;height:4px;width:100%;background:var(--accent);border-radius:2px}
.track b{position:absolute;right:-6px;top:-5px;width:14px;height:14px;border-radius:50%;background:var(--surface);border:2px solid var(--accent)}
/* select mode */
.card.sel{outline:3px solid var(--accent);outline-offset:3px;background:var(--surface-sel)}
.check{position:absolute;left:18px;top:18px;width:26px;height:26px;border:var(--input-border);border-radius:var(--radius-sm);background:var(--surface);display:flex;align-items:center;justify-content:center;padding:0;cursor:pointer;color:var(--accent-text);z-index:1}
.check.on{background:var(--accent-bg);border:var(--accent-border)}
.bulk{position:absolute;left:50%;transform:translateX(-50%);bottom:20px;display:flex;align-items:center;gap:6px;padding:8px 10px;border-radius:var(--radius-lg);background:var(--bulk-bg);color:var(--bulk-text);border:var(--pop-border);box-shadow:var(--pop-shadow);white-space:nowrap}
.bulk button{height:36px;padding:0 12px;border:0;border-radius:var(--radius);background:transparent;color:var(--bulk-text);font-family:var(--body);font-size:14px;font-weight:600;cursor:pointer;display:inline-flex;align-items:center;gap:8px}
.bulk button.main{background:var(--accent-bg);color:var(--accent-text)}
.bulk .vsep{width:1px;height:24px;background:var(--bulk-sep);margin:0 4px}
.bulk .n{font-size:11px;opacity:.7}
/* variant A extras */
.jump{position:absolute;right:18px;bottom:76px;display:flex;align-items:center;gap:8px;height:36px;padding:0 14px;border:var(--toast-border);border-radius:var(--radius-pill);background:var(--toast-bg);color:var(--toast-text);font-family:var(--body);font-size:13px;font-weight:600;box-shadow:var(--pop-shadow);white-space:nowrap;cursor:pointer;z-index:2}
.loadmore{display:flex;align-items:center;justify-content:center;gap:10px;height:44px;border:var(--load-border);border-radius:var(--radius);color:var(--muted);font-size:13px;background:var(--load-bg)}
.earlier{margin-top:6px;padding-top:14px;border-top:var(--earlier-rule)}
.card.dim{opacity:var(--dim-opacity)}
/* palettes */
.pal{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:20px;padding:20px;flex:1 1 auto}
.pcol{display:flex;flex-direction:column;gap:12px;padding:14px;border:var(--card-border);border-radius:var(--radius-lg);background:var(--surface);box-shadow:var(--card-shadow);color:var(--text)}
.pcol h3{margin:0;font-family:var(--display);font-size:15px}
.pcol .mini{border:var(--card-border);border-radius:var(--radius);overflow:hidden;background:var(--bg)}
.pcol .mini .titlebar{height:28px;font-size:11px}
.pcol .mini .titlebar .win span{width:30px;height:28px}
.pcol .mini .in{padding:10px;display:flex;flex-direction:column;gap:8px}
.swatches{display:flex;gap:6px}
.swatches span{flex:1;height:26px;border-radius:var(--radius-sm);border:1px solid rgba(0,0,0,.15)}
.hexes{font-family:var(--mono,var(--body));font-size:11px;color:var(--muted);line-height:1.5}
"""

# ------------------------------------------------------------------ styles
STYLES = {
 "Japandi": dict(
  fonts="family=Noto+Serif+JP:wght@400;600&family=Inter:wght@400;500;600",
  vars="""--display:'Noto Serif JP',serif;--body:Inter,system-ui,sans-serif;--mono:ui-monospace,Menlo,monospace;
--bg:#efe9df;--surface:#faf7f1;--surface-2:#f4efe6;--surface-sel:#ece4d6;--text:#2b2a26;--muted:#6b665c;
--accent:#7a5c3e;--accent-bg:#7a5c3e;--accent-text:#faf7f1;--accent-border:1px solid #7a5c3e;--accent-shadow:none;
--hair-w:1px;--hair-c:#d8cfbf;--hair:1px solid #d8cfbf;--frame-border:1px solid #cfc5b3;--frame-radius:10px;
--radius-sm:4px;--radius:6px;--radius-lg:10px;--radius-pill:999px;--dot-radius:50%;
--btn-border:1px solid #cdc3b1;--btn-bg:#faf7f1;--btn-text:#2b2a26;--btn-shadow:none;
--card-border:1px solid #d8cfbf;--card-shadow:0 1px 2px rgba(60,50,30,.06);--thumb:linear-gradient(135deg,#e6dccb,#d3c6b0);
--tag-bg:rgba(250,247,241,.9);--tag-text:#6b665c;--input-border:1px solid #cdc3b1;--input-bg:#fffdf9;
--chip-border:1px solid #cdc3b1;--chip-bg:transparent;--chip-text:#6b665c;
--toggle-border:1px solid #b9ad99;--toggle-off:#e9e1d3;--toggle-knob:#7a5c3e;
--title-bg:#e9e2d5;--title-text:#2b2a26;--label-case:none;--label-track:.02em;
--toast-bg:#2b2a26;--toast-text:#faf7f1;--toast-border:0;--pop-shadow:0 8px 24px rgba(60,50,30,.14);--pop-border:1px solid #d8cfbf;
--canvas:#e8e1d4;--shot-bg:#faf7f1;--shot-line:#d8cfbf;--shot-fill:#f4efe6;--shot-shadow:0 10px 30px rgba(60,50,30,.18);
--bulk-bg:#2b2a26;--bulk-text:#faf7f1;--bulk-sep:#5a564e;""",
  extra=".btn.primary{font-weight:500}h2.sec,.tab{letter-spacing:.01em}.titlebar .dot{background:#b5a48a}"),

 "Y2K": dict(
  fonts="family=Audiowide&family=Exo+2:wght@400;600;700",
  vars="""--display:Audiowide,sans-serif;--body:'Exo 2',sans-serif;--mono:'Exo 2',sans-serif;
--bg:linear-gradient(160deg,#dfe7ff 0%,#f6f0ff 50%,#d9f3ff 100%);--surface:linear-gradient(180deg,#ffffff,#eef2ff);--surface-2:rgba(255,255,255,.55);--surface-sel:#e6e9ff;--text:#1b1f3b;--muted:#4f5680;
--accent:#6a2bdc;--accent-bg:linear-gradient(180deg,#b388ff 0%,#6a2bdc 55%,#4e1bb3 100%);--accent-text:#fff;--accent-border:1px solid #4e1bb3;--accent-shadow:inset 0 1px 0 rgba(255,255,255,.7),0 2px 6px rgba(78,27,179,.4);
--hair-w:1px;--hair-c:#b9c2e6;--hair:1px solid #b9c2e6;--frame-border:2px solid #c7cff2;--frame-radius:18px;
--radius-sm:8px;--radius:14px;--radius-lg:20px;--radius-pill:999px;--dot-radius:50%;
--btn-border:1px solid #9aa6d8;--btn-bg:linear-gradient(180deg,#ffffff 0%,#dfe5fb 50%,#c9d2f5 51%,#e9edfd 100%);--btn-text:#1b1f3b;--btn-shadow:inset 0 1px 0 #fff,0 2px 4px rgba(27,31,59,.2);
--card-border:1px solid #c3cbef;--card-shadow:inset 0 1px 0 #fff,0 4px 12px rgba(27,31,59,.14);--thumb:linear-gradient(135deg,#ff9ae5,#9ad5ff 60%,#c7ffd8);
--tag-bg:rgba(255,255,255,.85);--tag-text:#1b1f3b;--input-border:1px solid #9aa6d8;--input-bg:linear-gradient(180deg,#eef1ff,#fff);
--chip-border:1px solid #9aa6d8;--chip-bg:linear-gradient(180deg,#fff,#e4e9ff);--chip-text:#1b1f3b;
--toggle-border:1px solid #9aa6d8;--toggle-off:linear-gradient(180deg,#e3e8ff,#c9d2f5);--toggle-knob:linear-gradient(180deg,#fff,#b9c2e6);
--title-bg:linear-gradient(180deg,#5d6fe0,#3545b8);--title-text:#fff;--label-case:uppercase;--label-track:.1em;
--toast-bg:linear-gradient(180deg,#ff9ae5,#e04fc1);--toast-text:#fff;--toast-border:1px solid #c33aa5;--pop-shadow:0 10px 30px rgba(27,31,59,.25);--pop-border:1px solid #9aa6d8;
--canvas:radial-gradient(circle at 30% 20%,#fff 0%,#dfe7ff 45%,#c9d6ff 100%);--shot-bg:#fff;--shot-line:#c3cbef;--shot-fill:#f3f5ff;--shot-shadow:0 14px 40px rgba(27,31,59,.3);
--bulk-bg:linear-gradient(180deg,#3545b8,#1f2b86);--bulk-text:#fff;--bulk-sep:#6b7ad6;""",
  extra=".titlebar .win span{border-left-color:rgba(255,255,255,.3)}.tab.on{text-shadow:0 0 8px rgba(106,43,220,.5)}.card::before{content:'';position:absolute;inset:0 0 50% 0;border-radius:inherit;background:linear-gradient(180deg,rgba(255,255,255,.75),rgba(255,255,255,0));pointer-events:none}.app::after{content:'';position:absolute;right:-120px;top:-120px;width:360px;height:360px;border-radius:50%;background:radial-gradient(circle,rgba(255,255,255,.9),rgba(255,255,255,0) 70%);pointer-events:none}"),

 "PixelArt": dict(
  fonts="family=Press+Start+2P&family=VT323",
  vars="""--display:'Press Start 2P',monospace;--body:VT323,monospace;--mono:VT323,monospace;
--bg:#1b1b2f;--surface:#2a2a45;--surface-2:#22223a;--surface-sel:#3a3a5e;--text:#f1f1e6;--muted:#a9a9c9;
--accent:#ffd23f;--accent-bg:#ffd23f;--accent-text:#1b1b2f;--accent-border:3px solid #1b1b2f;--accent-shadow:4px 4px 0 #0d0d18;
--hair-w:3px;--hair-c:#0d0d18;--hair:3px solid #0d0d18;--frame-border:4px solid #0d0d18;--frame-radius:0;
--radius-sm:0;--radius:0;--radius-lg:0;--radius-pill:0;--dot-radius:0;
--btn-border:3px solid #0d0d18;--btn-bg:#4a4a7a;--btn-text:#f1f1e6;--btn-shadow:4px 4px 0 #0d0d18;
--card-border:3px solid #0d0d18;--card-shadow:4px 4px 0 #0d0d18;--thumb:repeating-conic-gradient(#3d3d66 0 25%,#34345a 0 50%) 0 0/16px 16px;
--tag-bg:#0d0d18;--tag-text:#ffd23f;--input-border:3px solid #0d0d18;--input-bg:#15152a;
--chip-border:2px solid #0d0d18;--chip-bg:#4a4a7a;--chip-text:#f1f1e6;
--toggle-border:3px solid #0d0d18;--toggle-off:#4a4a7a;--toggle-knob:#f1f1e6;
--title-bg:#0d0d18;--title-text:#ffd23f;--label-case:uppercase;--label-track:.05em;
--toast-bg:#4fe3a3;--toast-text:#1b1b2f;--toast-border:3px solid #0d0d18;--pop-shadow:6px 6px 0 #0d0d18;--pop-border:3px solid #0d0d18;
--canvas:repeating-conic-gradient(#24243d 0 25%,#1f1f36 0 50%) 0 0/24px 24px;--shot-bg:#2a2a45;--shot-line:#5a5a8a;--shot-fill:#333355;--shot-shadow:8px 8px 0 #0d0d18;
--bulk-bg:#0d0d18;--bulk-text:#f1f1e6;--bulk-sep:#4a4a7a;""",
  extra="body{font-size:18px}.btn,.tab,.chip,.hint,.name,.field,.seg button,.opt,.corner button,.bulk button,.day span{font-size:17px}.hint{font-size:15px}.chip{font-size:14px}.label{font-size:10px}h2.sec,.tab.on{font-size:12px}.tab{font-size:11px}.titlebar{font-family:var(--display);font-size:10px}.tool .k{font-size:8px}.toggle::after{border-radius:0}.handle{border-radius:0}.sw.on{outline-offset:3px}.check{border-radius:0}.toast,.bulk{image-rendering:pixelated}.btn.primary,.toggle.on,.tab.on{font-family:var(--display);font-size:11px}.tab.on{font-size:11px}"),

 "Gothic": dict(
  fonts="family=Pirata+One&family=EB+Garamond:ital,wght@0,400;0,600;1,400",
  vars="""--display:'Pirata One',serif;--body:'EB Garamond',serif;--mono:'EB Garamond',serif;
--bg:#0d0a0f;--surface:#1a1419;--surface-2:#140f14;--surface-sel:#2a1c22;--text:#e8dcc6;--muted:#a3927c;
--accent:#9b1c2e;--accent-bg:linear-gradient(180deg,#b3233a,#7a1424);--accent-text:#f5ead6;--accent-border:1px solid #c9a35a;--accent-shadow:0 0 0 1px #2a1c22,0 6px 18px rgba(155,28,46,.45);
--hair-w:1px;--hair-c:#3a2d33;--hair:1px solid #3a2d33;--frame-border:2px solid #c9a35a;--frame-radius:2px;
--radius-sm:2px;--radius:3px;--radius-lg:4px;--radius-pill:3px;--dot-radius:0;
--btn-border:1px solid #5a4a4f;--btn-bg:#221a20;--btn-text:#e8dcc6;--btn-shadow:inset 0 0 0 1px #0d0a0f;
--card-border:1px solid #4a3a40;--card-shadow:0 0 0 1px #0d0a0f,0 8px 24px rgba(0,0,0,.6);--thumb:linear-gradient(160deg,#2b1f26,#120c10);
--tag-bg:rgba(13,10,15,.85);--tag-text:#c9a35a;--input-border:1px solid #5a4a4f;--input-bg:#120d11;
--chip-border:1px solid #5a4a4f;--chip-bg:#120d11;--chip-text:#c9a35a;
--toggle-border:1px solid #5a4a4f;--toggle-off:#221a20;--toggle-knob:#a3927c;
--title-bg:#07050a;--title-text:#c9a35a;--label-case:uppercase;--label-track:.18em;
--toast-bg:#1a1419;--toast-text:#e8dcc6;--toast-border:1px solid #c9a35a;--pop-shadow:0 12px 30px rgba(0,0,0,.7);--pop-border:1px solid #5a4a4f;
--canvas:radial-gradient(ellipse at center,#1b1419 0%,#0d0a0f 75%);--shot-bg:#1a1419;--shot-line:#3a2d33;--shot-fill:#221a20;--shot-shadow:0 20px 50px rgba(0,0,0,.8);
--bulk-bg:#120d11;--bulk-text:#e8dcc6;--bulk-sep:#3a2d33;""",
  extra="body{font-size:16px}h2.sec{font-size:24px;font-weight:400;letter-spacing:.02em}.tab{font-size:20px}.titlebar{font-family:var(--display);font-size:17px;letter-spacing:.04em}.label{font-size:11px;color:#c9a35a}.btn{font-weight:400;font-size:16px}.btn.primary{font-family:var(--display);letter-spacing:.04em;font-size:17px}.card::after{content:'';position:absolute;inset:4px;border:1px solid rgba(201,163,90,.25);border-radius:inherit;pointer-events:none}.handle{border-color:#c9a35a}.sw.on{outline-color:#c9a35a}.day::after{background:linear-gradient(90deg,#c9a35a,transparent)}"),

 "Kawaii": dict(
  fonts="family=Fredoka:wght@500;600&family=Nunito:wght@500;700",
  vars="""--display:Fredoka,sans-serif;--body:Nunito,sans-serif;--mono:Nunito,sans-serif;
--bg:#fff1f6;--surface:#ffffff;--surface-2:#fff7fa;--surface-sel:#ffe6ef;--text:#4b2a3b;--muted:#8d6478;
--accent:#c42a68;--accent-bg:#c42a68;--accent-text:#fff;--accent-border:2px solid #c42a68;--accent-shadow:0 4px 0 #8f1d4b;
--hair-w:2px;--hair-c:#ffd3e2;--hair:2px solid #ffd3e2;--frame-border:3px solid #ffc2d8;--frame-radius:26px;
--radius-sm:10px;--radius:14px;--radius-lg:22px;--radius-pill:999px;--dot-radius:50%;
--btn-border:2px solid #ffc2d8;--btn-bg:#fff;--btn-text:#4b2a3b;--btn-shadow:0 4px 0 #ffc2d8;
--card-border:2px solid #ffd3e2;--card-shadow:0 6px 0 #ffd3e2;--thumb:linear-gradient(135deg,#ffd6e7,#d9f0ff 55%,#fff5c2);
--tag-bg:#fff;--tag-text:#8d6478;--input-border:2px solid #ffd3e2;--input-bg:#fffafc;
--chip-border:2px solid #ffd3e2;--chip-bg:#fff;--chip-text:#8d6478;
--toggle-border:2px solid #ffc2d8;--toggle-off:#ffe6ef;--toggle-knob:#c42a68;
--title-bg:#ffd6e7;--title-text:#4b2a3b;--label-case:none;--label-track:0;
--toast-bg:#a3dcff;--toast-text:#1f3f55;--toast-border:2px solid #6fbcee;--pop-shadow:0 8px 0 #ffd3e2;--pop-border:2px solid #ffd3e2;
--canvas:radial-gradient(#ffd3e2 1.5px,transparent 1.5px) 0 0/22px 22px,#fff7fa;--shot-bg:#fff;--shot-line:#ffd3e2;--shot-fill:#fff7fa;--shot-shadow:0 10px 0 #ffd3e2;
--bulk-bg:#fff;--bulk-text:#4b2a3b;--bulk-sep:#ffd3e2;""",
  extra="body{font-size:15px}.btn{border-radius:999px;font-weight:700}.btn.primary{box-shadow:0 4px 0 #8f1d4b}.chip{font-weight:700}.tab{font-weight:600;font-size:16px}.tab.on{border-bottom-width:4px;border-radius:2px}h2.sec{font-size:20px}.titlebar .dot{background:#ff9ec4}.check{border-radius:50%}.toggle.on::after{box-shadow:0 0 0 2px #fff inset}.tool.on{border-radius:999px}"),

 "FrutigerAero": dict(
  fonts="family=Lexend:wght@400;500;600&family=Nunito+Sans:wght@400;600;700",
  vars="""--display:Lexend,sans-serif;--body:'Nunito Sans',sans-serif;--mono:'Nunito Sans',sans-serif;
--bg:linear-gradient(180deg,#bfe3f7 0%,#e9f6fb 40%,#d9f3e6 100%);--surface:linear-gradient(180deg,rgba(255,255,255,.95),rgba(236,247,252,.9));--surface-2:rgba(255,255,255,.5);--surface-sel:#e0f3ff;--text:#0f3350;--muted:#3f6a86;
--accent:#1a7fd0;--accent-bg:linear-gradient(180deg,#5fbaf5 0%,#1f82d6 45%,#146cb8 55%,#1f8ae0 100%);--accent-text:#fff;--accent-border:1px solid #156bb0;--accent-shadow:inset 0 1px 0 rgba(255,255,255,.8),0 3px 8px rgba(26,127,208,.4);
--hair-w:1px;--hair-c:#a9d4ec;--hair:1px solid #a9d4ec;--frame-border:1px solid #8fc5e6;--frame-radius:14px;
--radius-sm:6px;--radius:10px;--radius-lg:16px;--radius-pill:999px;--dot-radius:50%;
--btn-border:1px solid #8fc5e6;--btn-bg:linear-gradient(180deg,#ffffff 0%,#e8f5fc 48%,#d4ecf9 52%,#eef9ff 100%);--btn-text:#0f3350;--btn-shadow:inset 0 1px 0 #fff,0 2px 5px rgba(15,51,80,.18);
--card-border:1px solid #a9d4ec;--card-shadow:inset 0 1px 0 #fff,0 6px 16px rgba(15,51,80,.15);--thumb:linear-gradient(160deg,#9fe0ff,#4fc3f7 45%,#8be39a);
--tag-bg:rgba(255,255,255,.9);--tag-text:#0f3350;--input-border:1px solid #8fc5e6;--input-bg:linear-gradient(180deg,#f3fbff,#fff);
--chip-border:1px solid #8fc5e6;--chip-bg:linear-gradient(180deg,#fff,#e4f3fc);--chip-text:#0f3350;
--toggle-border:1px solid #8fc5e6;--toggle-off:linear-gradient(180deg,#eaf6fc,#cfe8f6);--toggle-knob:linear-gradient(180deg,#fff,#c9e3f2);
--title-bg:linear-gradient(180deg,#5cb8ef,#1f84cf);--title-text:#fff;--label-case:none;--label-track:0;
--toast-bg:linear-gradient(180deg,#9ff0a5,#3fbf55);--toast-text:#0b3a16;--toast-border:1px solid #2e9a42;--pop-shadow:0 10px 28px rgba(15,51,80,.25);--pop-border:1px solid #8fc5e6;
--canvas:linear-gradient(180deg,#d6eefb,#eaf8f0);--shot-bg:#fff;--shot-line:#a9d4ec;--shot-fill:#f0f9ff;--shot-shadow:0 16px 40px rgba(15,51,80,.3);
--bulk-bg:linear-gradient(180deg,#2f9ae8,#146bb4);--bulk-text:#fff;--bulk-sep:#7fc2ef;""",
  extra=".titlebar .win span{border-left-color:rgba(255,255,255,.35)}.card::before{content:'';position:absolute;inset:0 0 55% 0;border-radius:inherit;background:linear-gradient(180deg,rgba(255,255,255,.8),rgba(255,255,255,0));pointer-events:none}.app::before{content:'';position:absolute;left:-80px;bottom:-140px;width:420px;height:420px;border-radius:50%;background:radial-gradient(circle,rgba(139,227,154,.55),rgba(139,227,154,0) 70%);pointer-events:none}.tab.on{border-bottom-color:#3fbf55}"),

 "NeoBrutalism": dict(
  fonts="family=Archivo+Black&family=Space+Grotesk:wght@500;700",
  vars="""--display:'Archivo Black',sans-serif;--body:'Space Grotesk',sans-serif;--mono:'Space Grotesk',sans-serif;
--bg:#f3efe4;--surface:#ffffff;--surface-2:#f3efe4;--surface-sel:#fff;--text:#000;--muted:#333;
--accent:#000;--accent-bg:#ffde59;--accent-text:#000;--accent-border:3px solid #000;--accent-shadow:4px 4px 0 #000;
--hair-w:3px;--hair-c:#000;--hair:3px solid #000;--frame-border:4px solid #000;--frame-radius:0;
--radius-sm:0;--radius:0;--radius-lg:0;--radius-pill:0;--dot-radius:0;
--btn-border:3px solid #000;--btn-bg:#fff;--btn-text:#000;--btn-shadow:4px 4px 0 #000;
--card-border:3px solid #000;--card-shadow:6px 6px 0 #000;--thumb:#7cf5c2;
--tag-bg:#000;--tag-text:#fff;--input-border:3px solid #000;--input-bg:#fff;
--chip-border:2px solid #000;--chip-bg:#fff;--chip-text:#000;
--toggle-border:3px solid #000;--toggle-off:#fff;--toggle-knob:#000;
--title-bg:#ff6b6b;--title-text:#000;--label-case:uppercase;--label-track:.06em;
--toast-bg:#7cf5c2;--toast-text:#000;--toast-border:3px solid #000;--pop-shadow:6px 6px 0 #000;--pop-border:3px solid #000;
--canvas:repeating-linear-gradient(0deg,transparent 0 31px,#7fb3e0 31px 32px),repeating-linear-gradient(90deg,transparent 0 31px,#7fb3e0 31px 32px),#cfe8ff;--shot-bg:#fff;--shot-line:#000;--shot-fill:#f3efe4;--shot-shadow:10px 10px 0 #000;
--bulk-bg:#000;--bulk-text:#fff;--bulk-sep:#555;""",
  extra="body{font-size:15px;font-weight:500}.btn{font-weight:700;text-transform:uppercase;letter-spacing:.02em}.btn.sm{padding:0 9px;font-size:12px}.tab{font-size:15px;text-transform:uppercase}.tab.on{border-bottom:0;background:#ffde59;outline:3px solid #000;outline-offset:-3px}h2.sec{font-size:18px;text-transform:uppercase}.titlebar{font-family:var(--display);font-size:13px;text-transform:uppercase}.chip{font-weight:700;text-transform:uppercase}.chip.saved{background:#7cf5c2}.card:nth-child(3n+2) .thumb{background:#ffde59}.card:nth-child(3n) .thumb{background:#ff6b6b}.toggle::after{border-radius:0}.toggle.on{background:#7cf5c2}.toggle.on::after{background:#000}.seg button.on{background:#000;color:#fff}.tool.on{background:#000;color:#fff}.bulk button.main{background:#ffde59}.check{border-radius:0}.handle{border-radius:0;border-color:#000}.sw.on{outline-color:#000}.label{color:#000}"),
}

# ------------------------------------------------- deeper theming per style
# vars2: annotation ink, loading/earlier treatment. copy: micro-copy tone.
# extra2: window controls, icon rendering, textures. palettes: 3 colourways.
THEME = {
 "Japandi": dict(
  vars2="--focus:#2b2a26;--focus-halo:rgba(122,92,62,.25);--sticker:#c4855a;--sticker-line:#2b2a26;--anno:#7a5c3e;--load-border:1px dashed #cdc3b1;--load-bg:transparent;--earlier-rule:1px solid #d8cfbf;--dim-opacity:1;",
  copy=dict(loading="Fetching Tue, Nov 4 …", toast="Copied to clipboard", jump="Back to this session", earlier="Earlier — Thu, Nov 6", session="This session"),
  extra2=".titlebar .win span{color:#6b665c}.app svg{stroke-width:1.6}.thumb::after{content:'';position:absolute;inset:0;background:linear-gradient(180deg,transparent 60%,rgba(43,42,38,.08))}",
  palettes=[("Linen & walnut (current)", "--bg:#efe9df;--surface:#faf7f1;--surface-2:#f4efe6;--text:#2b2a26;--muted:#6b665c;--accent:#7a5c3e;--accent-bg:#7a5c3e;--accent-text:#faf7f1;--thumb:linear-gradient(135deg,#e6dccb,#d3c6b0);--title-bg:#e9e2d5;--title-text:#2b2a26;", ["#efe9df","#faf7f1","#7a5c3e","#2b2a26"]),
            ("Stone & moss", "--bg:#e9ebe6;--surface:#f6f7f4;--surface-2:#eff1ec;--text:#23261f;--muted:#5f665a;--accent:#4f6b4a;--accent-bg:#4f6b4a;--accent-text:#f6f7f4;--thumb:linear-gradient(135deg,#d7dccf,#bfc7b5);--title-bg:#dfe3da;--title-text:#23261f;", ["#e9ebe6","#f6f7f4","#4f6b4a","#23261f"]),
            ("Charcoal & clay (dark)", "--bg:#1f1e1b;--surface:#2a2824;--surface-2:#24231f;--text:#ece6da;--muted:#a69d8e;--accent:#c4855a;--accent-bg:#c4855a;--accent-text:#1f1e1b;--thumb:linear-gradient(135deg,#3a372f,#2e2b25);--title-bg:#161512;--title-text:#ece6da;--input-bg:#1a1916;--btn-bg:#2a2824;--btn-text:#ece6da;--card-border:1px solid #3d3a33;--hair:1px solid #3d3a33;--hair-c:#3d3a33;--input-border:1px solid #4a463e;--btn-border:1px solid #4a463e;--chip-text:#a69d8e;--chip-border:1px solid #4a463e;", ["#1f1e1b","#2a2824","#c4855a","#ece6da"])]),
 "Y2K": dict(
  vars2="--focus:#ff2fd0;--focus-halo:rgba(255,47,208,.25);--sticker:#ff9ae5;--sticker-line:#4e1bb3;--anno:#ff2fd0;--load-border:1px solid #9aa6d8;--load-bg:linear-gradient(180deg,#fff,#e4e9ff);--earlier-rule:1px solid #b9c2e6;--dim-opacity:1;",
  copy=dict(loading="Loading Tue, Nov 4 … ▰▰▰▱▱", toast="★ Copied to clipboard ★", jump="Back to this session", earlier="Earlier — Thu, Nov 6", session="This session"),
  extra2=".titlebar .win span{color:#fff}.titlebar .dot{background:radial-gradient(circle at 35% 30%,#fff,#ff9ae5 40%,#6a2bdc);box-shadow:0 0 8px rgba(255,154,229,.9)}.thumb::before{content:'';position:absolute;inset:0;background:linear-gradient(180deg,rgba(255,255,255,.75),rgba(255,255,255,0) 50%)}.btn,.chip,.toast,.jump{text-shadow:0 1px 0 rgba(255,255,255,.6)}.btn.primary,.toast,.jump,.bulk{text-shadow:0 1px 1px rgba(0,0,0,.35)}.tool.on{box-shadow:inset 0 2px 4px rgba(0,0,0,.35)}.app::before{content:'';position:absolute;left:10%;bottom:-200px;width:600px;height:400px;border-radius:50%;background:radial-gradient(ellipse,rgba(154,213,255,.6),rgba(154,213,255,0) 70%);pointer-events:none}",
  palettes=[("Chrome & violet (current)", "", ["#dfe7ff","#ffffff","#6a2bdc","#ff9ae5"]),
            ("Bubblegum & cyan", "--accent:#ff2fa6;--accent-bg:linear-gradient(180deg,#ffb3dc 0%,#ff2fa6 55%,#d5147f 100%);--accent-border:1px solid #d5147f;--title-bg:linear-gradient(180deg,#ff8ccb,#e01f8a);--bg:linear-gradient(160deg,#ffe3f3 0%,#e4fbff 100%);--thumb:linear-gradient(135deg,#ffb3dc,#7ff0ff);--toast-bg:linear-gradient(180deg,#8ff5ff,#19c4dc);--toast-text:#06303a;--toast-border:1px solid #0f9fb3;--bulk-bg:linear-gradient(180deg,#e01f8a,#9c0f5c);", ["#ffe3f3","#ffffff","#ff2fa6","#7ff0ff"]),
            ("Lime & graphite (dark)", "--bg:linear-gradient(160deg,#1c2130,#2a3350);--surface:linear-gradient(180deg,#39425e,#2b3350);--surface-2:rgba(255,255,255,.06);--text:#eef2ff;--muted:#aab3d6;--accent:#b6ff3b;--accent-bg:linear-gradient(180deg,#e2ff9a 0%,#b6ff3b 55%,#7fcc12 100%);--accent-text:#132000;--accent-border:1px solid #7fcc12;--title-bg:linear-gradient(180deg,#4a5680,#2a3350);--thumb:linear-gradient(135deg,#b6ff3b,#3bd9ff);--btn-bg:linear-gradient(180deg,#4f5a80,#39425e);--btn-text:#eef2ff;--btn-border:1px solid #6b78a8;--input-bg:#222a40;--input-border:1px solid #6b78a8;--card-border:1px solid #4f5a80;--hair:1px solid #4f5a80;--hair-c:#4f5a80;--chip-bg:#39425e;--chip-text:#eef2ff;--chip-border:1px solid #6b78a8;--toast-bg:linear-gradient(180deg,#e2ff9a,#7fcc12);--toast-text:#132000;", ["#1c2130","#39425e","#b6ff3b","#3bd9ff"])]),
 "PixelArt": dict(
  vars2="--focus:#4fe3a3;--focus-halo:transparent;--sticker:#ffd23f;--sticker-line:#0d0d18;--anno:#ff6b97;--load-border:3px dashed #4a4a7a;--load-bg:#15152a;--earlier-rule:3px dashed #4a4a7a;--dim-opacity:1;",
  copy=dict(loading="LOADING NOV 4 ▮▮▮▯▯▯", toast="★ COPIED! ★", jump="BACK TO SESSION", earlier="EARLIER — NOV 6", session="THIS SESSION"),
  extra2=".app svg{shape-rendering:crispEdges;stroke-linecap:square;stroke-linejoin:miter;stroke-width:2.5}.titlebar .win span{color:#ffd23f}.titlebar .win span::before{content:'';width:10px;height:10px;background:currentColor;box-shadow:none}.titlebar .win span{font-size:0}.thumb .tag{font-family:var(--body);font-size:14px;padding:2px 6px}.settings{font-size:16px}.settings .row>span,.settings label:not(.label){font-size:16px}.thumb .tag{font-size:13px;white-space:nowrap;max-width:calc(100% - 16px);overflow:hidden;text-overflow:ellipsis}.hint{font-size:15px}.card{box-shadow:4px 4px 0 #0d0d18,inset -3px -3px 0 #1c1c30}.btn{box-shadow:4px 4px 0 #0d0d18,inset -3px -3px 0 rgba(0,0,0,.35),inset 3px 3px 0 rgba(255,255,255,.12)}.toast,.jump{font-family:var(--display);font-size:10px}.loadmore{font-family:var(--display);font-size:10px}.day h3{font-size:10px}.anno.text{font-family:var(--display)!important;font-size:11px!important}",
  palettes=[("Night arcade (current)", "", ["#1b1b2f","#2a2a45","#ffd23f","#4fe3a3"]),
            ("Game Boy green", "--bg:#0f380f;--surface:#306230;--surface-2:#1e4a1e;--surface-sel:#4a7a3a;--text:#e0f8d0;--muted:#9bbc0f;--accent:#9bbc0f;--accent-bg:#9bbc0f;--accent-text:#0f380f;--accent-border:3px solid #0f380f;--thumb:repeating-conic-gradient(#8bac0f 0 25%,#306230 0 50%) 0 0/16px 16px;--title-bg:#0a2a0a;--title-text:#9bbc0f;--btn-bg:#306230;--btn-text:#e0f8d0;--input-bg:#0f380f;--chip-bg:#306230;--toast-bg:#9bbc0f;--toast-text:#0f380f;--tag-bg:#0f380f;--tag-text:#9bbc0f;--hair-c:#0a2a0a;--hair:3px solid #0a2a0a;--btn-border:3px solid #0a2a0a;--card-border:3px solid #0a2a0a;--input-border:3px solid #0a2a0a;--chip-border:2px solid #0a2a0a;--toggle-border:3px solid #0a2a0a;--toggle-off:#306230;--toggle-knob:#e0f8d0;", ["#0f380f","#306230","#9bbc0f","#e0f8d0"]),
            ("CRT amber", "--bg:#2b1d0e;--surface:#3d2a14;--surface-2:#33230f;--surface-sel:#5a3d1c;--text:#ffe6b3;--muted:#d9a85c;--accent:#ff9f1c;--accent-bg:#ff9f1c;--accent-text:#2b1d0e;--accent-border:3px solid #140c04;--thumb:repeating-conic-gradient(#4d3519 0 25%,#3d2a14 0 50%) 0 0/16px 16px;--title-bg:#140c04;--title-text:#ff9f1c;--btn-bg:#5a3d1c;--btn-text:#ffe6b3;--input-bg:#1f1408;--chip-bg:#5a3d1c;--toast-bg:#ffd166;--toast-text:#2b1d0e;--tag-bg:#140c04;--tag-text:#ff9f1c;--hair-c:#140c04;--hair:3px solid #140c04;--btn-border:3px solid #140c04;--card-border:3px solid #140c04;--input-border:3px solid #140c04;--chip-border:2px solid #140c04;--toggle-border:3px solid #140c04;--toggle-off:#5a3d1c;--toggle-knob:#ffe6b3;", ["#2b1d0e","#3d2a14","#ff9f1c","#ffe6b3"])]),
 "Gothic": dict(
  vars2="--focus:#c9a35a;--focus-halo:rgba(201,163,90,.25);--sticker:#c9a35a;--sticker-line:#0d0a0f;--anno:#c9a35a;--load-border:1px solid #3a2d33;--load-bg:#120d11;--earlier-rule:1px solid #3a2d33;--dim-opacity:1;",
  copy=dict(loading="Unearthing Tuesday, the 4th …", toast="Copied to the clipboard", jump="Return to this session", earlier="Earlier — Thursday, the 6th", session="This Session"),
  extra2=".titlebar .win span{color:#c9a35a;border-left-color:#2a1c22}.titlebar .dot{width:10px;height:10px;transform:rotate(45deg);background:#c9a35a}.app svg{stroke-width:1.5}.tab.on{border-bottom-color:#c9a35a}.toggle.on{background:#9b1c2e}.toggle.on::after{background:#c9a35a}.thumb{box-shadow:inset 0 0 0 1px rgba(201,163,90,.3),inset 0 0 40px rgba(0,0,0,.6)}.toast,.jump{font-family:var(--display);font-size:16px;letter-spacing:.04em}.app{background:radial-gradient(ellipse at top,#1a1419,#0d0a0f 70%)}.chip.saved{background:linear-gradient(180deg,#b3233a,#7a1424);color:#f5ead6}.bulk button.main{background:linear-gradient(180deg,#b3233a,#7a1424);color:#f5ead6}.bulk{border:1px solid #c9a35a}",
  palettes=[("Crimson & gilt (current)", "", ["#0d0a0f","#1a1419","#9b1c2e","#c9a35a"]),
            ("Nightshade violet", "--accent:#6b2fa0;--accent-bg:linear-gradient(180deg,#7e3bbd,#4a1f73);--accent-border:1px solid #b9a0d8;--title-text:#b9a0d8;--tag-text:#b9a0d8;--chip-text:#b9a0d8;--label-color:#b9a0d8;--frame-border:2px solid #b9a0d8;--toast-border:1px solid #b9a0d8;--thumb:linear-gradient(160deg,#2a1f33,#120c16);--bg:#0b0810;--surface:#17121d;--surface-2:#110d16;", ["#0b0810","#17121d","#6b2fa0","#b9a0d8"]),
            ("Bone & ash (light)", "--bg:#e9e3d8;--surface:#f4f0e8;--surface-2:#ece6da;--surface-sel:#ddd4c3;--text:#1e1a1c;--muted:#5e5357;--accent:#5a1020;--accent-bg:linear-gradient(180deg,#7a1424,#4a0b16);--accent-text:#f4f0e8;--accent-border:1px solid #1e1a1c;--title-bg:#1e1a1c;--title-text:#c9a35a;--btn-bg:#f4f0e8;--btn-text:#1e1a1c;--btn-border:1px solid #9a8e86;--input-bg:#fbf8f2;--input-border:1px solid #9a8e86;--card-border:1px solid #9a8e86;--hair:1px solid #c9bfb0;--hair-c:#c9bfb0;--chip-bg:#f4f0e8;--chip-text:#5a1020;--chip-border:1px solid #9a8e86;--thumb:linear-gradient(160deg,#d8cfbf,#bfb3a0);--tag-bg:rgba(30,26,28,.85);--toast-bg:#1e1a1c;--toast-text:#f4f0e8;--toggle-off:#ddd4c3;--toggle-knob:#5e5357;--canvas:radial-gradient(ellipse at center,#f4f0e8 0%,#ddd4c3 75%);--shot-bg:#fbf8f2;--shot-line:#c9bfb0;--shot-fill:#ece6da;--bulk-bg:#1e1a1c;--bulk-text:#f4f0e8;", ["#e9e3d8","#f4f0e8","#5a1020","#1e1a1c"])]),
 "Kawaii": dict(
  vars2="--focus:#c42a68;--focus-halo:rgba(196,42,104,.25);--sticker:#ffe08a;--sticker-line:#4b2a3b;--anno:#c42a68;--load-border:2px dashed #ffc2d8;--load-bg:#fff;--earlier-rule:2px dashed #ffd3e2;--dim-opacity:1;",
  copy=dict(loading="Loading Tue, Nov 4 … ♡", toast="Copied! ♡", jump="Back to this session", earlier="Earlier — Thu, Nov 6 ✿", session="This session ✦"),
  extra2=".titlebar .win span{border-left:0;width:38px}.titlebar .win span{font-size:0}.titlebar .win span::before{content:'';width:14px;height:14px;border-radius:50%;background:#ff9ec4;box-shadow:inset 0 -2px 0 rgba(0,0,0,.12)}.titlebar .win span:nth-child(2)::before{background:#ffe08a}.titlebar .win span:nth-child(3)::before{background:#a3dcff}.app svg{stroke-width:2.4;stroke-linecap:round;stroke-linejoin:round}.thumb::after{content:'';position:absolute;right:10px;top:10px;width:22px;height:22px;border-radius:50%;background:rgba(255,255,255,.7)}.btn.sm{border-radius:999px}.toast,.jump{border-radius:999px}.tool{border-radius:999px}.check{width:28px;height:28px}",
  palettes=[("Strawberry milk (current)", "", ["#fff1f6","#ffffff","#c42a68","#a3dcff"]),
            ("Mint soda", "--bg:#eefcf6;--surface:#fff;--surface-2:#f4fdf8;--surface-sel:#dff7ea;--text:#1f4a3a;--muted:#5d8a77;--accent:#17a673;--accent-bg:#17a673;--accent-text:#fff;--accent-border:2px solid #17a673;--accent-shadow:0 4px 0 #0f7a54;--hair:2px solid #bdeedb;--hair-c:#bdeedb;--frame-border:3px solid #a9e8cf;--btn-border:2px solid #bdeedb;--btn-shadow:0 4px 0 #bdeedb;--card-border:2px solid #bdeedb;--card-shadow:0 6px 0 #bdeedb;--thumb:linear-gradient(135deg,#c8f5e2,#fff6c2 55%,#d9e8ff);--input-border:2px solid #bdeedb;--chip-border:2px solid #bdeedb;--chip-text:#5d8a77;--toggle-border:2px solid #a9e8cf;--toggle-off:#dff7ea;--toggle-knob:#17a673;--title-bg:#c8f5e2;--toast-bg:#ffe08a;--toast-text:#4a3a05;--toast-border:2px solid #e6c05a;--pop-shadow:0 8px 0 #bdeedb;--pop-border:2px solid #bdeedb;--canvas:radial-gradient(#bdeedb 1.5px,transparent 1.5px) 0 0/22px 22px,#f4fdf8;--shot-line:#bdeedb;--shot-fill:#f4fdf8;--shot-shadow:0 10px 0 #bdeedb;--bulk-sep:#bdeedb;", ["#eefcf6","#ffffff","#17a673","#ffe08a"]),
            ("Lavender night (dark)", "--bg:#2a2340;--surface:#372e52;--surface-2:#302846;--surface-sel:#4a3f6b;--text:#f3edff;--muted:#bfb3e0;--accent:#c58bff;--accent-bg:#c58bff;--accent-text:#2a2340;--accent-border:2px solid #c58bff;--accent-shadow:0 4px 0 #8d5fd1;--hair:2px solid #4a3f6b;--hair-c:#4a3f6b;--frame-border:3px solid #5b4f80;--btn-bg:#372e52;--btn-text:#f3edff;--btn-border:2px solid #5b4f80;--btn-shadow:0 4px 0 #1d1830;--card-border:2px solid #5b4f80;--card-shadow:0 6px 0 #1d1830;--thumb:linear-gradient(135deg,#8d5fd1,#ff9ec4 55%,#ffe08a);--input-bg:#2a2340;--input-border:2px solid #5b4f80;--chip-bg:#372e52;--chip-text:#bfb3e0;--chip-border:2px solid #5b4f80;--toggle-border:2px solid #5b4f80;--toggle-off:#4a3f6b;--toggle-knob:#c58bff;--title-bg:#1d1830;--title-text:#f3edff;--tag-bg:#2a2340;--tag-text:#f3edff;--toast-bg:#ffe08a;--toast-text:#4a3a05;--toast-border:2px solid #e6c05a;--pop-shadow:0 8px 0 #1d1830;--pop-border:2px solid #5b4f80;--canvas:radial-gradient(#4a3f6b 1.5px,transparent 1.5px) 0 0/22px 22px,#302846;--shot-bg:#372e52;--shot-line:#5b4f80;--shot-fill:#302846;--shot-shadow:0 10px 0 #1d1830;--bulk-bg:#372e52;--bulk-text:#f3edff;--bulk-sep:#5b4f80;", ["#2a2340","#372e52","#c58bff","#ffe08a"])]),
 "FrutigerAero": dict(
  vars2="--focus:#0f3350;--focus-halo:rgba(26,127,208,.3);--sticker:#8be39a;--sticker-line:#0f3350;--anno:#1a7fd0;--load-border:1px solid #8fc5e6;--load-bg:linear-gradient(180deg,#fff,#e4f3fc);--earlier-rule:1px solid #a9d4ec;--dim-opacity:1;",
  copy=dict(loading="Loading Tuesday, Nov 4 …", toast="Copied to clipboard", jump="Back to this session", earlier="Earlier — Thursday, Nov 6", session="This session"),
  extra2=".titlebar .win span{color:#fff}.titlebar .dot{background:radial-gradient(circle at 35% 30%,#fff,#7fd0ff 40%,#1a7fd0);box-shadow:0 0 6px rgba(127,208,255,.9)}.thumb::before{content:'';position:absolute;inset:0;background:linear-gradient(180deg,rgba(255,255,255,.7),rgba(255,255,255,0) 50%)}.thumb::after{content:'';position:absolute;right:-20px;bottom:-30px;width:90px;height:90px;border-radius:50%;background:radial-gradient(circle,rgba(255,255,255,.9),rgba(255,255,255,0) 70%)}.btn.primary,.bulk{text-shadow:0 1px 1px rgba(0,0,0,.3)}.toast,.jump{border-radius:999px;text-shadow:none}.tool.on{box-shadow:inset 0 2px 4px rgba(0,0,0,.3)}.app::after{content:'';position:absolute;right:-60px;top:40px;width:260px;height:260px;border-radius:50%;background:radial-gradient(circle at 40% 35%,rgba(255,255,255,.95),rgba(255,255,255,.1) 60%,rgba(255,255,255,0) 70%);pointer-events:none}",
  palettes=[("Sky & grass (current)", "", ["#bfe3f7","#ffffff","#1a7fd0","#3fbf55"]),
            ("Aqua & coral", "--bg:linear-gradient(180deg,#bff2f0 0%,#eafcfb 40%,#ffe9e0 100%);--accent:#0ea5a5;--accent-bg:linear-gradient(180deg,#8ff0ea 0%,#1fbdbd 48%,#0ea5a5 52%,#2fd0cf 100%);--accent-border:1px solid #0a8585;--title-bg:linear-gradient(180deg,#4fd6d2,#0ea5a5);--thumb:linear-gradient(160deg,#a9f3ee,#2fd0cf 45%,#ffb59c);--toast-bg:linear-gradient(180deg,#ffc5b0,#ff7a59);--toast-text:#4a1a0c;--toast-border:1px solid #d65a3c;--bulk-bg:linear-gradient(180deg,#1fbdbd,#0a8585);--hair:1px solid #a4e4e2;--hair-c:#a4e4e2;--card-border:1px solid #a4e4e2;--btn-border:1px solid #86d6d4;--input-border:1px solid #86d6d4;--chip-border:1px solid #86d6d4;--toggle-border:1px solid #86d6d4;--text:#0f3d3d;--muted:#3f7a7a;", ["#bff2f0","#ffffff","#0ea5a5","#ff7a59"]),
            ("Midnight glass (dark)", "--bg:linear-gradient(180deg,#0f2a44 0%,#143a5c 50%,#0f3a3a 100%);--surface:linear-gradient(180deg,rgba(40,80,120,.9),rgba(20,55,90,.9));--surface-2:rgba(255,255,255,.06);--surface-sel:#1f4f78;--text:#e8f4ff;--muted:#9cc3e0;--accent:#39c6ff;--accent-bg:linear-gradient(180deg,#a8e6ff 0%,#39c6ff 48%,#1aa8e6 52%,#4fd0ff 100%);--accent-text:#042033;--accent-border:1px solid #1486bd;--title-bg:linear-gradient(180deg,#1f5f94,#0f3a5e);--thumb:linear-gradient(160deg,#39c6ff,#1aa8e6 45%,#4fe08a);--btn-bg:linear-gradient(180deg,#3b6a95,#24496e);--btn-text:#e8f4ff;--btn-border:1px solid #4f85b8;--input-bg:#143652;--input-border:1px solid #4f85b8;--card-border:1px solid #3b6a95;--hair:1px solid #2d5a85;--hair-c:#2d5a85;--chip-bg:#24496e;--chip-text:#e8f4ff;--chip-border:1px solid #4f85b8;--toggle-off:#24496e;--toggle-knob:#9cc3e0;--toggle-border:1px solid #4f85b8;--tag-bg:rgba(4,32,51,.85);--tag-text:#e8f4ff;--toast-bg:linear-gradient(180deg,#9ff0a5,#3fbf55);--canvas:linear-gradient(180deg,#143a5c,#0f3a3a);--shot-bg:#1a3d5c;--shot-line:#3b6a95;--shot-fill:#24496e;--bulk-bg:linear-gradient(180deg,#1f5f94,#0f3a5e);", ["#0f2a44","#24496e","#39c6ff","#4fe08a"])]),
 "NeoBrutalism": dict(
  vars2="--focus:#4d7cff;--focus-halo:transparent;--sticker:#ffde59;--sticker-line:#000;--anno:#000;--load-border:3px dashed #000;--load-bg:#fff;--earlier-rule:3px solid #000;--dim-opacity:1;",
  copy=dict(loading="LOADING TUE NOV 4 →", toast="COPIED.", jump="BACK TO THIS SESSION", earlier="EARLIER — THU NOV 6", session="THIS SESSION"),
  extra2=".app svg{stroke-width:2.5;stroke-linecap:square;stroke-linejoin:miter}.titlebar .win span{font-weight:900;border-left:3px solid #000}.titlebar .dot{width:16px;height:16px;background:#000}.card.dim{background:#f3efe4}.toast,.jump{font-family:var(--display);font-size:13px;text-transform:uppercase}.loadmore{font-weight:700}.tab{font-weight:700}.day h3{text-transform:uppercase}.day::after{height:3px}.card:nth-child(3n+2) .thumb{background:#ffde59}.card:nth-child(3n) .thumb{background:#ff6b6b}.thumb{border:3px solid #000}.anno.text{text-transform:uppercase;font-family:var(--display)!important}",
  palettes=[("Cream / yellow / mint / coral (current)", "", ["#f3efe4","#ffde59","#7cf5c2","#ff6b6b"]),
            ("Paper / blue / lime / orange", "--bg:#fffdf5;--surface-2:#fffdf5;--accent-bg:#4d7cff;--accent-text:#fff;--surface-sel:#4d7cff;--title-bg:#ff9f1c;--thumb:#c6ff5c;--toast-bg:#c6ff5c;--bulk-sep:#555;", ["#fffdf5","#4d7cff","#c6ff5c","#ff9f1c"]),
            ("Black / acid / white (dark)", "--bg:#121212;--surface:#1e1e1e;--surface-2:#121212;--surface-sel:#d4ff00;--text:#fff;--muted:#cfcfcf;--accent:#fff;--accent-bg:#d4ff00;--accent-text:#000;--accent-border:3px solid #fff;--accent-shadow:4px 4px 0 #fff;--hair:3px solid #fff;--hair-c:#fff;--frame-border:4px solid #fff;--btn-border:3px solid #fff;--btn-bg:#1e1e1e;--btn-text:#fff;--btn-shadow:4px 4px 0 #fff;--card-border:3px solid #fff;--card-shadow:6px 6px 0 #fff;--thumb:#ff4fd8;--tag-bg:#fff;--tag-text:#000;--input-border:3px solid #fff;--input-bg:#1e1e1e;--chip-border:2px solid #fff;--chip-bg:#1e1e1e;--chip-text:#fff;--toggle-border:3px solid #fff;--toggle-off:#1e1e1e;--toggle-knob:#fff;--title-bg:#d4ff00;--title-text:#000;--toast-bg:#d4ff00;--toast-border:3px solid #fff;--pop-shadow:6px 6px 0 #fff;--pop-border:3px solid #fff;--canvas:repeating-linear-gradient(0deg,transparent 0 31px,#444 31px 32px),repeating-linear-gradient(90deg,transparent 0 31px,#444 31px 32px),#1e1e1e;--shot-bg:#121212;--shot-line:#fff;--shot-fill:#1e1e1e;--shot-shadow:10px 10px 0 #fff;--bulk-bg:#fff;--bulk-text:#000;--bulk-sep:#999;--load-border:3px dashed #fff;--load-bg:#1e1e1e;--earlier-rule:3px solid #fff;", ["#121212","#d4ff00","#ff4fd8","#ffffff"])]),
}

# --------------------------------------------------------------- fragments
SVG = {
 "region": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M4 8V4h4M20 8V4h-4M4 16v4h4M20 16v4h-4"/></svg>',
 "window": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/></svg>',
 "screen": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></svg>',
 "clock": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>',
 "plus": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>',
 "gear": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-2-3.5-2.3 1a7 7 0 0 0-1.7-1L14.5 3h-5l-.4 2.6a7 7 0 0 0-1.7 1l-2.3-1-2 3.5 2 1.5a7 7 0 0 0 0 2l-2 1.5 2 3.5 2.3-1a7 7 0 0 0 1.7 1l.4 2.6h5l.4-2.6a7 7 0 0 0 1.7-1l2.3 1 2-3.5-2-1.5c.1-.3.1-.7.1-1z"/></svg>',
 "check": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" aria-hidden="true"><path d="M5 12l5 5L20 7"/></svg>',
 "copy": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5h10"/></svg>',
 "back": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>',
 "undo": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3"/></svg>',
 "redo": '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3"/></svg>',
 "paths": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h10"/></svg>',
 "save": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2zM17 21v-8H7v8M7 3v5h8"/></svg>',
 "merge": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 7l9 6 9-6M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V7M3 7l9-4 9 4"/></svg>',
 "trash": '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14M10 11v6M14 11v6"/></svg>',
 "hand": '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M8 13V6a1.5 1.5 0 0 1 3 0v5M11 11V4.5a1.5 1.5 0 0 1 3 0V11M14 11V6a1.5 1.5 0 0 1 3 0v7.5a6 6 0 0 1-6 6h-.6a6 6 0 0 1-5.1-2.9L3.4 13a1.6 1.6 0 0 1 2.7-1.7L8 13.5"/></svg>',
}
TOOLS = [
 ("Select (V)", "V", '<path d="M5 3l14 9-6 1.5L10 20z"/>', False),
 ("Pan (H)", "H", '<path d="M8 13V5a1.5 1.5 0 0 1 3 0v6M11 10V4a1.5 1.5 0 0 1 3 0v7M14 11V6a1.5 1.5 0 0 1 3 0v8a6 6 0 0 1-6 6h-1a6 6 0 0 1-5-2.7L3 13a1.6 1.6 0 0 1 2.7-1.7L8 14"/>', False),
 None,
 ("Crop (C)", "C", '<path d="M6 2v14a2 2 0 0 0 2 2h14M2 6h14a2 2 0 0 1 2 2v14"/>', False),
 ("Block / blur (B)", "B", '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9l6-6M3 15L15 3M9 21L21 9M15 21l6-6"/>', False),
 None,
 ("Arrow (A)", "A", '<path d="M5 19L19 5M19 5h-8M19 5v8"/>', True),
 ("Line (L)", "L", '<path d="M5 19L19 5"/>', False),
 ("Rectangle (R)", "R", '<rect x="4" y="5" width="16" height="14" rx="2"/>', False),
 ("Ellipse (O)", "O", '<ellipse cx="12" cy="12" rx="8" ry="7"/>', False),
 None,
 ("Draw (P)", "P", '<path d="M12 19l7-7 3 3-7 7-3-3zM18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5zM2 2l7.6 7.6"/>', False),
 ("Highlighter (M)", "M", '<path d="M9 11l-6 6v3h9l3-3M22 12l-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"/>', False),
 ("Text (T)", "T", '<path d="M4 7V4h16v3M9 20h6M12 4v16"/>', False),
 ("Number badge (N)", "N", '<circle cx="12" cy="12" r="9"/><path d="M10 9l2-1.5V16"/>', False),
 ("Stickers (S)", "S", '<path d="M8 13V6a1.5 1.5 0 0 1 3 0v5M11 11V4.5a1.5 1.5 0 0 1 3 0V11M14 11V6a1.5 1.5 0 0 1 3 0v7.5a6 6 0 0 1-6 6h-.6a6 6 0 0 1-5.1-2.9L3.4 13a1.6 1.6 0 0 1 2.7-1.7L8 13.5"/>', False),
 None,
 ("Eraser (E)", "E", '<path d="M20 20H8L3 15a2 2 0 0 1 0-3l9-9a2 2 0 0 1 3 0l6 6a2 2 0 0 1 0 3l-7 7M6.5 11.5l6 6"/>', False),
]

def titlebar(text):
    return f'''<div class="titlebar"><div style="display:flex;align-items:center;gap:10px"><span class="dot" aria-hidden="true"></span><span>{text}</span></div><div class="win" aria-hidden="true"><span>–</span><span>▢</span><span>✕</span></div></div>'''

def toolbar():
    return f'''<div class="toolbar">
<button class="btn primary">{SVG["region"]}Region</button>
<button class="btn kbd-focus" aria-describedby="fx">{SVG["window"]}Window</button><span id="fx" class="srlabel">Shown with keyboard focus ring</span>
<button class="btn">{SVG["screen"]}Full screen<span class="sub">Display 1 ▾</span></button>
<button class="btn">{SVG["clock"]}Delayed<span class="sub">5 s ▾</span></button>
<span class="hint" style="margin-left:4px">PrtScn · Ctrl+Shift+S</span>
<div style="flex:1 1 40px"></div>
<button class="btn">{SVG["plus"]}New session</button>
<button class="btn icon" aria-label="Settings">{SVG["gear"]}</button>
</div>'''

def card(tag, name, saved, copied=False, cls="", check=None):
    chip = '<span class="chip saved">Saved ✓</span>' if saved else '<span class="chip">Unsaved</span>'
    if check is None:
        if copied:
            cp = f'<button class="btn sm" aria-label="Copied" style="width:60px;justify-content:center">{SVG["check"]}</button>'
        else:
            cp = '<button class="btn sm">Copy</button>'
        act = f'{cp}<button class="btn sm">Open folder</button>' if saved else f'{cp}<button class="btn sm primary">Save</button>'
        act += '<button class="btn sm icon" aria-label="More">⋯</button>'
        tail = f'<div class="row">{chip}<div style="display:flex;gap:6px">{act}</div></div>'
        chk = ""
    else:
        tail = f'<div class="row">{chip}<span class="hint">{"…\\Screenshots\\" + name + ".png" if saved else "no path yet"}</span></div>'
        chk = (f'<button class="check on" aria-label="Selected" aria-pressed="true">{SVG["check"]}</button>' if check
               else '<button class="check" aria-label="Not selected" aria-pressed="false"></button>')
    return f'''<div class="card {cls}">{chk}<div class="thumb"><span class="tag">{tag}</span></div>
<input class="name" value="{name}" aria-label="Screenshot name">{tail}</div>'''

def toggle_row(label, on, idn):
    return f'''<div class="row"><label for="{idn}" style="font-size:14px">{label}</label><button id="{idn}" class="toggle{" on" if on else ""}" role="switch" aria-checked="{"true" if on else "false"}" aria-label="{label}"></button></div>'''

def settings():
    return f'''<div class="settings">
<div class="group"><span class="label">Session</span>
{toggle_row("Show previous screenshots", True, "t0")}
<p class="hint" style="margin:-4px 0 0">Off = clean slate each session. On = earlier unsaved shots load below, a day at a time as you scroll.</p>
</div>
<div class="group"><span class="label">Capture</span>
{toggle_row("Copy to clipboard on capture", True, "t1")}
{toggle_row("Auto-save every capture", False, "t2")}
{toggle_row("Keep running in the tray", True, "t3")}
{toggle_row("Open editor after capture", False, "t4")}
</div>
<div class="group"><span class="label">Shortcuts</span>
<div class="row"><span>PrtScn</span><span class="hint">Region (default)</span></div>
<div class="row"><span>Ctrl + Shift + W</span><span class="hint">Window</span></div>
<div class="row"><span>Ctrl + Shift + F</span><span class="hint">Full screen</span></div>
<button class="btn sm" style="align-self:flex-start">Edit shortcuts…</button>
</div>
<div class="group"><span class="label">Save to</span>
<div style="display:flex;gap:8px"><div class="field" style="flex:1 1 0">C:\\Users\\Pat\\Pictures\\Screenshots</div><button class="btn sm">Change</button></div>
</div>
<div class="group"><label class="label" for="pat">Name pattern</label>
<input id="pat" class="name" value="screenshot_{{mm}}_{{dd}}_{{yyyy}}">
<p class="hint" style="margin:0">Same-day captures get _2, _3 … appended.</p>
</div>
<div class="group"><span class="label">Format</span>
<div class="seg" role="group" aria-label="Image format"><button class="on">PNG</button><button>JPG</button><button>WebP</button></div>
</div>
</div>'''

def scene_main(title, cp):
    cards = "".join([
        card("1440×900 · Region · 2:31 PM", "screenshot_11_07_2026_3", False, copied=True),
        card("820×610 · Window · 2:22 PM", "login-bug-repro", True),
        card("2560×1440 · Full screen · 2:15 PM", "screenshot_11_07_2026", False),
    ])
    earlier = "".join([
        card("1200×700 · Region · Nov 6, 4:48 PM", "screenshot_11_06_2026_2", False, cls="dim"),
        card("640×420 · Window · Nov 6, 4:40 PM", "screenshot_11_06_2026", False, cls="dim"),
    ])
    return f'''<div class="app">
{titlebar("Snapshot — session started 2:14 PM")}
{toolbar()}
<div class="body">
<div class="gallery" style="padding:16px 18px 64px;position:relative">
<button class="jump">{SVG["check"].replace('M5 12l5 5L20 7','M12 19V5M5 12l7-7 7 7')}{cp["jump"]}</button>
<div class="row"><div style="display:flex;align-items:baseline;gap:10px"><h2 class="sec">{cp["session"]}</h2><span class="hint">3 captures · 1 saved · nothing written to disk until you save</span></div>
<div style="display:flex;gap:8px"><div class="field" style="height:32px;width:180px;color:var(--muted)">Search names…</div><button class="btn sm">Select</button></div></div>
<div class="grid">{cards}</div>
<div class="row earlier"><div style="display:flex;align-items:baseline;gap:10px"><h2 class="sec" style="color:var(--muted)">{cp["earlier"]}</h2><span class="hint">2 unsaved · shown because “Show previous” is on</span></div><button class="btn sm">Collapse ▴</button></div>
<div class="grid">{earlier}</div>
<div class="loadmore" aria-live="polite"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M21 12a9 9 0 1 1-3-6.7"/></svg>{cp["loading"]}</div>
</div>
{settings()}
</div>
<div class="toast" role="status">{SVG["check"]}{cp["toast"]}</div>
<div class="bottom"><span class="hint">3 captures · 1 saved · 2 unsaved — Save all skips anything already saved.</span>
<div style="display:flex;gap:8px"><button class="btn">Clear unsaved</button><button class="btn">Copy all</button><button class="btn primary">Save all (2 new)</button></div></div>
</div>'''

def scene_editor(title, cp):
    tools = ""
    for t in TOOLS:
        if t is None:
            tools += '<div class="sep"></div>'; continue
        lab, k, path, on = t
        tools += f'<button class="tool{" on" if on else ""}" aria-label="{lab}"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">{path}</svg><span class="k">{k}</span></button>'
    return f'''<div class="app">
{titlebar("Snapshot — editing screenshot_11_07_2026_3")}
<div class="toolbar">
<a href="#" class="btn">{SVG["back"]}Back to session</a>
<input class="name" style="width:240px" value="screenshot_11_07_2026_3" aria-label="Screenshot name">
<span class="hint">1440×900 · edited · not saved</span>
<div style="flex:1 1 40px"></div>
<button class="btn">Reset edits</button><button class="btn">{SVG["copy"]}Copy</button><button class="btn primary">Save</button><button class="btn">Save as ▾</button>
</div>
<div class="canvas">
<div class="pill" style="position:absolute;left:50%;transform:translateX(-50%);top:16px" role="toolbar" aria-label="Tools">{tools}</div>
<div class="panel" aria-label="Arrow properties">
<div class="group"><span class="label">Stroke</span><div style="display:flex;gap:8px">
<button class="sw on" style="background:var(--text)" aria-label="Ink"></button><button class="sw" style="background:#fff" aria-label="White"></button><button class="sw" style="background:var(--anno)" aria-label="Red"></button><button class="sw" style="background:#f1c40f" aria-label="Yellow"></button><button class="sw" style="background:#2e86de" aria-label="Blue"></button></div></div>
<div class="group"><span class="label">Width</span><div style="display:flex;gap:6px">
<button class="opt" aria-label="Thin"><span style="display:block;width:14px;height:1px;background:currentColor"></span></button><button class="opt on" aria-label="Medium"><span style="display:block;width:14px;height:3px;background:currentColor"></span></button><button class="opt" aria-label="Thick"><span style="display:block;width:14px;height:5px;background:currentColor"></span></button></div></div>
<div class="group"><span class="label">Arrow head</span><div style="display:flex;gap:6px;flex-wrap:wrap"><button class="opt on">→</button><button class="opt">↔</button><button class="opt">⟶</button><button class="opt">↝</button></div></div>
<div class="group"><span class="label">Style</span><div style="display:flex;gap:6px"><button class="opt on">Solid</button><button class="opt">Dashed</button><button class="opt">Sketchy</button></div></div>
<div class="group"><span class="label">Opacity</span><div class="track"><i></i><b></b></div></div>
</div>
<div class="shot">
<div class="ui" style="left:60px;top:60px;width:640px;height:44px"></div>
<div class="ui" style="left:60px;top:130px;width:300px;height:280px"></div>
<div class="ui" style="left:400px;top:130px;width:300px;height:130px"></div>
<div class="ui" style="left:400px;top:290px;width:300px;height:120px"></div>
<div class="anno" style="left:388px;top:118px;width:324px;height:154px;border:3px solid var(--anno);border-radius:4px">
<div class="handle" style="left:-7px;top:-7px"></div><div class="handle" style="right:-7px;top:-7px"></div><div class="handle" style="left:-7px;bottom:-7px"></div><div class="handle" style="right:-7px;bottom:-7px"></div></div>
<svg class="anno" style="left:200px;top:40px" width="200" height="110" viewBox="0 0 200 110" fill="none" stroke="var(--anno)" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M10 100 C60 90 120 70 185 20"/><path d="M160 20 L185 20 L180 44"/></svg>
<div class="anno text" style="left:90px;top:140px;padding:6px 10px;border:2px dashed var(--anno);background:var(--shot-bg);font-size:16px;font-weight:700;color:var(--anno)">This total is wrong</div>
<div class="anno" style="left:372px;top:102px;width:28px;height:28px;border-radius:50%;background:var(--anno);color:#fff;font-size:14px;font-weight:700;display:flex;align-items:center;justify-content:center">1</div>
<div class="anno" style="left:410px;top:300px;width:280px;height:100px;backdrop-filter:blur(6px);background:repeating-linear-gradient(45deg,rgba(128,128,128,.5) 0 6px,rgba(160,160,160,.5) 6px 12px);border-radius:4px;display:flex;align-items:center;justify-content:center"><span class="tag" style="position:static">Blurred</span></div>
<svg class="anno" style="left:300px;top:330px" width="64" height="64" viewBox="0 0 24 24" fill="var(--sticker)" stroke="var(--sticker-line)" stroke-width="1.2" stroke-linejoin="round" role="img" aria-label="Pointing hand sticker"><path d="M8 13V6a1.5 1.5 0 0 1 3 0v5M11 11V4.5a1.5 1.5 0 0 1 3 0V11M14 11V6a1.5 1.5 0 0 1 3 0v7.5a6 6 0 0 1-6 6h-.6a6 6 0 0 1-5.1-2.9L3.4 13a1.6 1.6 0 0 1 2.7-1.7L8 13.5"/></svg>
</div>
<div class="corner" style="left:16px;bottom:16px"><button aria-label="Zoom out">−</button><button style="min-width:56px">53%</button><button aria-label="Zoom in">+</button><div class="sep"></div><button>Fit</button></div>
<div class="corner" style="right:16px;bottom:16px"><button aria-label="Undo">{SVG["undo"]}</button><button aria-label="Redo">{SVG["redo"]}</button></div>
<span class="hint" style="position:absolute;left:50%;transform:translateX(-50%);bottom:24px;padding:6px 12px;border-radius:var(--radius);background:var(--surface);border:var(--pop-border);box-shadow:var(--pop-shadow)">Drag to draw an arrow · Shift keeps it straight · Esc back to Select</span>
</div></div>'''

def scene_select(title, cp):
    cards = "".join([
        card("1440×900 · Region · 2:31 PM", "screenshot_11_07_2026_6", False, cls="sel", check=True),
        card("820×610 · Window · 2:22 PM", "login-bug-repro", True, cls="sel", check=True),
        card("2560×1440 · Full screen · 2:15 PM", "screenshot_11_07_2026_4", False, check=False),
        card("1200×700 · Region · 2:14 PM", "screenshot_11_07_2026_3", True, cls="sel", check=True),
        card("900×600 · Window · 2:12 PM", "screenshot_11_07_2026_2", False, check=False),
        card("640×420 · Region · 2:10 PM", "screenshot_11_07_2026", False, check=False),
    ])
    return f'''<div class="app" style="min-height:700px">
<div class="gallery" style="flex:1 1 auto;padding-top:10px">
<div class="tabs"><div style="display:flex" role="tablist"><button class="tab on" role="tab" aria-selected="true">This session (6)</button><button class="tab" role="tab" aria-selected="false">History</button></div>
<div style="display:flex;gap:8px;padding-bottom:6px;align-items:center"><span class="hint">Select mode · Shift-click for a range · Ctrl+A for all</span><button class="btn sm">Select all</button><button class="btn sm primary">Done</button></div></div>
<div class="grid" style="padding-bottom:90px">{cards}</div>
</div>
<div class="bulk" role="toolbar" aria-label="Bulk actions">
<span style="padding:0 8px;font-weight:700">3 selected</span><span class="n" style="padding-right:6px">2 saved · 1 unsaved</span><div class="vsep"></div>
<button class="main">{SVG["copy"]}Copy as files</button>
<button>{SVG["paths"]}Copy paths <span class="n">(2)</span></button>
<button>{SVG["save"]}Save <span class="n">(1 new)</span></button>
<button>{SVG["merge"]}Merge ▾</button><div class="vsep"></div>
<button>{SVG["trash"]}Delete</button><button aria-label="Clear selection" style="padding:0 10px">✕</button>
</div></div>'''

def scene_palettes(title, cp, palettes=None):
    cols = ""
    for name, overrides, hexes in palettes:
        sw = "".join(f'<span style="background:{h}"></span>' for h in hexes)
        cols += f'''<div class="pcol" style="{overrides}">
<h3>{name}</h3>
<div class="swatches">{sw}</div>
<div class="hexes">{" · ".join(hexes)}</div>
<div class="mini">{titlebar("Snapshot")}
<div class="in">
<div style="display:flex;gap:6px"><button class="btn sm primary">{SVG["region"]}Region</button><button class="btn sm">Window</button><button class="btn sm icon" aria-label="Settings">{SVG["gear"]}</button></div>
<div class="card" style="padding:8px;gap:6px"><div class="thumb" style="height:70px"><span class="tag">1440×900</span></div><input class="name" value="screenshot_11_07_2026" aria-label="Name"><div class="row"><span class="chip">Unsaved</span><span class="chip saved">Saved ✓</span></div></div>
<div class="row"><span style="font-size:13px">Show previous</span><button class="toggle on" role="switch" aria-checked="true" aria-label="Show previous"></button></div>
<div class="toast" style="position:static;transform:none;height:34px;font-size:13px;align-self:flex-start">{SVG["check"]}{cp["toast"]}</div>
</div></div></div>'''
    return f'''<div class="app" style="min-height:560px">{titlebar("Colour palettes — pick one, or ask for a custom mix")}<div class="pal">{cols}</div></div>'''

SCENES = [("Main", "Main window (A)", scene_main, 1280, 940),
          ("Editor", "Editor", scene_editor, 1280, 820),
          ("Select", "Select mode", scene_select, 1000, 700),
          ("Palettes", "Colour palettes", scene_palettes, 1000, 560)]

def page(style, vars_, extra, fonts, body, title, w, h):
    return f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?{fonts}&display=swap">
<style>
:root{{{vars_}}}
{BASE_CSS}
{extra}
</style>
</helmet>
{body}
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":{w},"height":{h}}}}}'>
class Component extends DCLogic {{
  renderVals() {{ return {{}}; }}
}}
</script>
</body>
</html>
'''

boards, order, notes = {}, [], {}
LABELS = {"Japandi":"Japandi","Y2K":"Y2K","PixelArt":"Pixel art","Gothic":"Gothic","Kawaii":"Kawaii","FrutigerAero":"Neo Frutiger Aero","NeoBrutalism":"Neo-Brutalism"}
y = 4700
for sname, st in STYLES.items():
    notes[f"style-{sname}"] = {"x": 0, "y": y - 240, "text": LABELS[sname], "kind": "title1", "maxW": 1800}
    x = 0
    th = THEME[sname]
    for key, label, fn, w, h in SCENES:
        fname = f"styles/{sname}-{key}.dc.html"
        title = f"{LABELS[sname]} — {label}"
        body = fn(title, th["copy"], palettes=th["palettes"]) if key == "Palettes" else fn(title, th["copy"])
        (OUT / f"{sname}-{key}.dc.html").write_text(page(sname, st["vars"] + th["vars2"], st["extra"] + th["extra2"], st["fonts"], body, title, w, h), encoding="utf-8")
        boards[fname] = {"x": x, "y": y, "w": w, "h": h, "title": title, "expand": "fill"}
        order.append(fname)
        x += w + 80
    y += 940 + 360

json.dump({"boards": boards, "order": order, "notes": notes}, open(ROOT / "gen" / "layout.json", "w"), indent=1)
print(len(order), "files")
