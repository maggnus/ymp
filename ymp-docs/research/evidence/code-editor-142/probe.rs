use ratatui::{layout::Rect, Terminal, backend::TestBackend};
use ratatui_code_editor::{editor::Editor, theme::vesper, actions::{InsertText,Undo,Redo}};
use crossterm::event::{KeyCode,KeyEvent,KeyEventKind,KeyModifiers};
use serde_json::json;
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
 let area=Rect::new(0,0,30,8);
 let mut observations=Vec::new();
 for (name,text) in [("unicode_lf","fn main() {\n    let привет = \"日 👩‍💻 e\u{301}\";  \n}\n"),("crlf","a\r\nb\r\n"),("no_final_newline","a  b"),("controls","a\t\u{1b}[2Jb\u{7}c\n")] {
  let mut e=Editor::new("rust",text,vesper())?;
  let mut t=Terminal::new(TestBackend::new(30,8))?;t.draw(|f|f.render_widget(&e,area))?;
  assert_eq!(e.get_content(),text);
  e.apply(InsertText{text:"X".into()});assert!(e.get_content().starts_with('X'));
  e.apply(Undo{});assert_eq!(e.get_content(),text);
  e.apply(Redo{});assert!(e.get_content().starts_with('X'));
  observations.push(json!({"case":name,"roundtrip_undo_redo":true}));
 }
 for (label,text) in [("ascii","x".repeat(50)),("wide","界".repeat(12)),("emoji","👩‍💻".repeat(12))] {
  let mut e=Editor::new("text",&text,vesper())?;e.set_cursor(text.chars().count());e.focus(&area);
  observations.push(json!({"case":label,"cursor":e.get_cursor(),"offset_x":e.get_offset_x(),"visible_cursor":e.get_visible_cursor(&area)}));
 }
 let mut e=Editor::new("text","",vesper())?;
 e.input(KeyEvent::new_with_kind(KeyCode::Char('a'),KeyModifiers::NONE,KeyEventKind::Press),&area)?;
 e.input(KeyEvent::new_with_kind(KeyCode::Char('a'),KeyModifiers::NONE,KeyEventKind::Release),&area)?;
 observations.push(json!({"case":"press_and_release","content":e.get_content()}));
 let mut e=Editor::new("text","first\r\nsecond\r\n",vesper())?;e.set_cursor(5);e.input(KeyEvent::new(KeyCode::Enter,KeyModifiers::NONE),&area)?;
 observations.push(json!({"case":"enter_crlf","content":e.get_content()}));
 let source="fn main() { let value = 42; }\n".repeat(8192);let start=Instant::now();let e=Editor::new("rust",&source,vesper())?;let create=start.elapsed();let mut t=Terminal::new(TestBackend::new(80,24))?;let start=Instant::now();t.draw(|f|f.render_widget(&e,f.area()))?;
 observations.push(json!({"case":"bounded_file","bytes":source.len(),"create_ms":create.as_millis(),"first_draw_ms":start.elapsed().as_millis()}));
 let mut plain=Editor::new("text","plain text",vec![("text","#000000"),("default","#000000")])?;
 plain.set_word_highlight_enabled(false);let mut t=Terminal::new(TestBackend::new(40,8))?;
 t.draw(|f|{f.render_widget(ratatui::widgets::Block::default().style(ratatui::style::Style::default().fg(ratatui::style::Color::Black).bg(ratatui::style::Color::White)),f.area());f.render_widget(&plain,f.area());})?;
 let cell=t.backend().buffer().content.iter().find(|c|c.symbol()=="p").unwrap();
 observations.push(json!({"case":"light_plain_text","foreground":format!("{:?}",cell.fg),"background":format!("{:?}",cell.bg)}));
 for (language,source) in [("markdown","# Heading\n```rust\nfn main() {}\n```\n"),("diff","--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n"),("text","a\tb\n")] {
  let e=Editor::new(language,source,vesper())?;let mut t=Terminal::new(TestBackend::new(60,9))?;t.draw(|f|f.render_widget(&e,f.area()))?;
  let buffer=t.backend().buffer();let rows:Vec<String>=(0..9).map(|y|(0..60).map(|x|buffer[(x,y)].symbol()).collect::<String>().trim_end().to_string()).collect();
  observations.push(json!({"case":format!("render_{language}"),"has_grammar":e.code_ref().is_highlight(),"rows":rows}));
 }
 println!("{}",serde_json::to_string_pretty(&observations)?);Ok(())
}
