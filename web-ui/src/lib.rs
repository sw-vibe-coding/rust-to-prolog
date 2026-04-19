use web_sys::{HtmlSelectElement, HtmlTextAreaElement, KeyboardEvent};
use yew::prelude::*;

pub mod demos;
pub mod runner;

use demos::DEMOS;
use runner::{run_source, RunOutcome};

pub enum Msg {
    SelectDemo(usize),
    SourceChanged(String),
    Run,
    Reset,
    Clear,
    KeyDown(KeyboardEvent),
}

pub struct App {
    selected: usize,
    source: String,
    output: String,
    status: String,
    error: bool,
}

impl App {
    fn load_demo(&mut self, idx: usize) {
        if let Some(demo) = DEMOS.get(idx) {
            self.selected = idx;
            self.source = demo.source.to_string();
            self.output.clear();
            self.status = "idle".into();
            self.error = false;
        }
    }

    fn run(&mut self) {
        let outcome: RunOutcome = run_source(&self.source);
        self.output = outcome.output;
        self.status = outcome.verdict;
        self.error = outcome.error;
    }
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        let idx = demos::default_index();
        let demo = &DEMOS[idx];
        Self {
            selected: idx,
            source: demo.source.to_string(),
            output: String::new(),
            status: "idle".into(),
            error: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SelectDemo(i) => {
                self.load_demo(i);
                true
            }
            Msg::SourceChanged(v) => {
                self.source = v;
                false
            }
            Msg::Run => {
                self.run();
                true
            }
            Msg::Reset => {
                let idx = self.selected;
                self.load_demo(idx);
                true
            }
            Msg::Clear => {
                self.output.clear();
                self.status = "idle".into();
                self.error = false;
                true
            }
            Msg::KeyDown(e) => {
                if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) {
                    e.prevent_default();
                    ctx.link().send_message(Msg::Run);
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_demo = ctx.link().callback(|e: Event| {
            let target: HtmlSelectElement = e.target_unchecked_into();
            let idx: usize = target.value().parse().unwrap_or(0);
            Msg::SelectDemo(idx)
        });
        let on_src = ctx.link().callback(|e: InputEvent| {
            let target: HtmlTextAreaElement = e.target_unchecked_into();
            Msg::SourceChanged(target.value())
        });
        let on_run = ctx.link().callback(|_| Msg::Run);
        let on_reset = ctx.link().callback(|_| Msg::Reset);
        let on_clear = ctx.link().callback(|_| Msg::Clear);
        let on_keydown = ctx.link().callback(Msg::KeyDown);

        let status_class = if self.error {
            "status status-error"
        } else {
            "status"
        };

        html! {
            <>
            <a href="https://github.com/sw-vibe-coding/rust-to-prolog" class="github-corner"
               aria-label="View source on GitHub" target="_blank">
                <svg width="80" height="80" viewBox="0 0 250 250" aria-hidden="true">
                    <path d="M0,0 L115,115 L130,115 L142,142 L250,250 L250,0 Z" />
                    <path d="M128.3,109.0 C113.8,99.7 119.0,89.6 119.0,89.6 C122.0,82.7 120.5,78.6 \
                        120.5,78.6 C119.2,72.0 123.4,76.3 123.4,76.3 C127.3,80.9 125.5,87.3 125.5,87.3 \
                        C122.9,97.6 130.6,101.9 134.4,103.2" fill="currentColor"
                        style="transform-origin:130px 106px;" class="octo-arm" />
                    <path d="M115.0,115.0 C114.9,115.1 118.7,116.5 119.8,115.4 L133.7,101.6 C136.9,99.2 \
                        139.9,98.4 142.2,98.6 C133.8,88.0 127.5,74.4 143.8,58.0 C148.5,53.4 154.0,51.2 \
                        159.7,51.0 C160.3,49.4 163.2,43.6 171.4,40.1 C171.4,40.1 176.1,42.5 178.8,56.2 \
                        C183.1,58.6 187.2,61.8 190.9,65.4 C194.5,69.0 197.7,73.2 200.1,77.6 C213.8,80.2 \
                        216.3,84.9 216.3,84.9 C212.7,93.1 206.9,96.0 205.4,96.6 C205.1,102.4 203.0,107.8 \
                        198.3,112.5 C181.9,128.9 168.3,122.5 157.7,114.1 C157.9,116.9 156.7,120.9 \
                        152.7,124.9 L141.0,136.5 C139.8,137.7 141.6,141.9 141.8,141.8 Z"
                        fill="currentColor" />
                </svg>
            </a>
            <main class="page" onkeydown={on_keydown.clone()}>
                <header class="chrome">
                    <h1>{ "rust-to-prolog" }</h1>
                    <div class="controls">
                        <select onchange={on_demo}>
                            { for DEMOS.iter().enumerate().map(|(i, d)| html! {
                                <option value={i.to_string()} selected={i == self.selected}>
                                    { d.name }
                                </option>
                            })}
                        </select>
                        <button onclick={on_run}>{ "Run" }</button>
                        <button class="secondary" onclick={on_reset}>{ "Reset" }</button>
                        <button class="secondary" onclick={on_clear}>{ "Clear" }</button>
                    </div>
                </header>
                <div class="split">
                    <section class="panel">
                        <label>{ "source (.pl)" }</label>
                        <textarea
                            class="src"
                            rows="22"
                            spellcheck="false"
                            value={self.source.clone()}
                            oninput={on_src}
                            onkeydown={on_keydown.clone()}
                        />
                    </section>
                    <section class="panel">
                        <label>{ "output" }</label>
                        <div class={status_class}>
                            { format!("status: {}", self.status) }
                        </div>
                        <pre class="out">{ &self.output }</pre>
                    </section>
                </div>
                <section class="panel about">
                    <p>
                        { "Full Rust pipeline (tokenize → parse → compile → emit → asm → refvm) \
                           running in WASM. No COR24 emulator. Hit " }
                        <kbd>{ "Cmd+Enter" }</kbd>
                        { " (or " }
                        <kbd>{ "Ctrl+Enter" }</kbd>
                        { ") to run. See " }
                        <a href="https://github.com/sw-vibe-coding/rust-to-prolog/blob/main/docs/rationale.md" target="_blank">{ "rationale" }</a>
                        { " / " }
                        <a href="https://github.com/sw-vibe-coding/rust-to-prolog/blob/main/docs/demos.md" target="_blank">{ "demos" }</a>
                        { " / " }
                        <a href="https://github.com/sw-vibe-coding/rust-to-prolog/blob/main/docs/limitations.md" target="_blank">{ "limitations" }</a>
                        { " for the design and scope." }
                    </p>
                </section>
            </main>
            <footer>
                <span>{"MIT License"}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{"\u{00a9} 2026 Michael A Wright"}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://makerlisp.com" target="_blank">{"COR24-TB"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://software-wrighter-lab.github.io/" target="_blank">{"Blog"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://discord.com/invite/Ctzk5uHggZ" target="_blank">{"Discord"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://www.youtube.com/@SoftwareWrighter" target="_blank">{"YouTube"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://github.com/sw-embed/sw-cor24-prolog" target="_blank">{"Port target"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://github.com/sw-vibe-coding/rust-to-prolog/blob/main/docs/demos.md" target="_blank">{"Demo Documentation"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://github.com/sw-vibe-coding/rust-to-prolog" target="_blank">{"GitHub"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{ format!("{} \u{00b7} {} \u{00b7} {}",
                    env!("BUILD_HOST"),
                    env!("BUILD_SHA"),
                    env!("BUILD_TIMESTAMP"),
                ) }</span>
            </footer>
            </>
        }
    }
}
