//! The app's stylesheet. Colours are tokens on `:root`, with a dark variant that follows the system setting.
pub const CSS: &str = r#"
:root {
  --bg: #f6f7f9; --panel: #ffffff; --text: #14181f; --muted: #566070; --line: #d5dae2;
  --accent: #1f5fbf; --accent-text: #ffffff; --danger: #b3261e; --ok: #1b6e3a; --warn-bg: #fff4d6;
  --focus: #1f5fbf; --row-on: #e3edfb; --toast: #14181f; --toast-text: #ffffff;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #14171c; --panel: #1c2027; --text: #e8ebf0; --muted: #a2acba; --line: #333a45;
    --accent: #6ea4ff; --accent-text: #0b1220; --danger: #ff8a80; --ok: #7ddc9b; --warn-bg: #3a3118;
    --focus: #8bb6ff; --row-on: #26334a; --toast: #e8ebf0; --toast-text: #14171c;
  }
}
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; }
body { background: var(--bg); color: var(--text); font: 15px/1.45 system-ui, "Segoe UI", Roboto, sans-serif; }
.app { height: 100vh; display: flex; flex-direction: column; }
h1 { font-size: 1.15rem; margin: 0; }
h2 { font-size: 1.3rem; margin: 0; }
h3 { font-size: 1rem; margin: 1.4rem 0 .4rem; }
button, input, textarea { font: inherit; color: inherit; }
button {
  background: var(--panel); border: 1px solid var(--line); border-radius: 6px; padding: .4rem .8rem; cursor: pointer;
}
button:hover:not(:disabled) { border-color: var(--accent); }
button:disabled { opacity: .5; cursor: default; }
button.primary { background: var(--accent); color: var(--accent-text); border-color: var(--accent); font-weight: 600; }
button.danger { color: var(--danger); }
button.link { border: none; background: none; color: var(--accent); text-decoration: underline; padding: .2rem; }
button.wide { width: calc(100% - 1.2rem); margin: .6rem; }
:focus-visible { outline: 3px solid var(--focus); outline-offset: 2px; }
input, textarea {
  background: var(--panel); border: 1px solid var(--line); border-radius: 6px; padding: .45rem .6rem; width: 100%;
}
input[type=checkbox] { width: auto; }
label { display: block; margin: .8rem 0 .25rem; font-weight: 600; font-size: .9rem; }
label.check { font-weight: 400; margin-top: 1rem; }
.mono { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
.big { font-size: 1.5rem; letter-spacing: .08em; }
.muted { color: var(--muted); } .small { font-size: .85rem; } .pad { padding: 1rem; }
.error { color: var(--danger); } .ok { color: var(--ok); }
.row { display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; } .row.tight { gap: .3rem; margin-left: auto; }
.grow { flex: 1; }

.centre { max-width: 34rem; margin: 6vh auto; padding: 1.5rem; }
.centre h1 { font-size: 1.6rem; margin-bottom: .6rem; }
.centre .primary { margin-top: 1.2rem; padding: .6rem 1.4rem; }
.warn { background: var(--warn-bg); padding: .6rem .8rem; border-radius: 6px; }
.passphrase { font: 1.35rem/1.5 ui-monospace, Menlo, Consolas, monospace; padding: .8rem; background: var(--panel);
  border: 1px solid var(--line); border-radius: 6px; word-break: break-word; }
.label { font-weight: 600; margin-bottom: .2rem; }
.meter { color: var(--muted); font-size: .85rem; margin: .3rem 0; }
.hint { color: var(--muted); font-size: .85rem; margin: 0; }

.bar { display: flex; gap: 1rem; align-items: center; padding: .55rem 1rem; background: var(--panel); border-bottom: 1px solid var(--line); }
.bar input[type=search] { max-width: 22rem; }
.bar nav { display: flex; gap: .25rem; margin-left: auto; }
.tab { border-color: transparent; background: none; }
.tab.on { background: var(--row-on); border-color: var(--accent); }
.toast { position: fixed; bottom: 1rem; left: 50%; transform: translateX(-50%); background: var(--toast); color: var(--toast-text);
  padding: .6rem 1rem; border-radius: 8px; z-index: 5; box-shadow: 0 4px 16px #0004; }
.toast.error { background: var(--danger); color: #fff; }

.split { flex: 1; display: grid; grid-template-columns: minmax(240px, 340px) 1fr; min-height: 0; }
.split aside { border-right: 1px solid var(--line); overflow: auto; background: var(--panel); }
.detail { overflow: auto; }
.list { list-style: none; margin: 0; padding: 0; }
.row { }
button.row { display: flex; flex-direction: column; align-items: flex-start; width: 100%; border: none; border-radius: 0;
  border-bottom: 1px solid var(--line); background: none; padding: .6rem 1rem; text-align: left; gap: 0; }
button.row.on { background: var(--row-on); }
.title { font-weight: 600; } .sub { color: var(--muted); font-size: .85rem; }

.head { display: flex; justify-content: space-between; align-items: center; gap: 1rem; flex-wrap: wrap; margin-bottom: 1rem; }
dl { margin: 0; }
.field { display: grid; grid-template-columns: 11rem 1fr; gap: .5rem; padding: .55rem 0; border-bottom: 1px solid var(--line); align-items: center; }
.field dt { color: var(--muted); font-size: .9rem; }
.field dd { margin: 0; display: flex; gap: .6rem; align-items: center; min-width: 0; }
.field dd > span:first-child { word-break: break-word; min-width: 0; }
.field dd .row.tight { flex-wrap: nowrap; flex-shrink: 0; }
.notes { white-space: pre-wrap; }
.mask { letter-spacing: .1em; color: var(--muted); }
.tag { background: var(--row-on); border-radius: 999px; padding: .1rem .6rem; margin-right: .3rem; font-size: .85rem; }
.history { list-style: none; padding: 0; margin: 0; }
.history li { display: flex; justify-content: space-between; align-items: center; gap: 1rem; padding: .35rem 0; border-bottom: 1px solid var(--line); }

.form { max-width: 40rem; margin: 0 auto; padding: 1.2rem 1.5rem 3rem; overflow: auto; height: calc(100vh - 3.5rem); }
@media (max-width: 820px) { .split { grid-template-columns: 1fr; } .field { grid-template-columns: 1fr; } }
"#;
