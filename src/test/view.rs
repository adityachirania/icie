use crate::{test::TestRun, STATUS};
use std::{
	fs, ops::{Deref, DerefMut}, sync::{Mutex, MutexGuard}
};

lazy_static::lazy_static! {
	pub static ref WEBVIEW: Mutex<Option<evscode::Webview>> = Mutex::new(None);
}

fn handle_events(stream: evscode::Future<evscode::Cancellable<json::JsonValue>>) {
	let _status = STATUS.push("Watching testview");
	for note in stream {
		match note["tag"].as_str() {
			Some("trigger_rr") => evscode::spawn(move || crate::debug::rr(note["in_path"].as_str().unwrap())),
			Some("new_test") => evscode::spawn(move || crate::test::add(note["input"].as_str().unwrap(), note["desired"].as_str().unwrap())),
			_ => log::error!("unrecognied testview webview food `{}`", note.dump()),
		}
	}
}

pub fn prepare_webview<'a>(lck: &'a mut MutexGuard<Option<evscode::Webview>>) -> &'a evscode::Webview {
	let requires_create = lck.as_ref().map(|webview| webview.was_disposed().wait()).unwrap_or(true);
	if requires_create {
		let webview: evscode::Webview = evscode::Webview::new("icie.test.view", "ICIE Test view", evscode::ViewColumn::Beside)
			.enable_scripts()
			.retain_context_when_hidden()
			.create();
		let stream = webview.listener().cancel_on(webview.disposer());
		evscode::spawn(move || Ok(handle_events(stream)));
		*MutexGuard::deref_mut(lck) = Some(webview);
	}
	MutexGuard::deref(lck).as_ref().unwrap()
}
pub fn webview_exists() -> evscode::R<bool> {
	let lck = WEBVIEW.lock()?;
	Ok(if let Some(webview) = &*lck { !webview.was_disposed().wait() } else { false })
}

pub fn render(tests: &[TestRun]) -> evscode::R<String> {
	Ok(format!(
		r#"
		<html>
			<head>
				<style>{css}</style>
				<link href="https://fonts.googleapis.com/icon?family=Material+Icons" rel="stylesheet">
				<script>{js}</script>
			</head>
			<body>
				<table class="test-table">
					{test_table}
				</table>
				<br/>
				<div id="new-container" class="new">
					<textarea id="new-input" class="new"></textarea>
					<textarea id="new-desired" class="new"></textarea>
					<div id="new-start" class="material-icons button new" onclick="new_start()">add</div>
					<div id="new-confirm" class="material-icons button new" onclick="new_confirm()">done</div>
				</div>
			</body>
		</html>
	"#,
		css = include_str!("view.css"),
		js = include_str!("view.js"),
		test_table = render_test_table(tests)?
	))
}

fn render_test_table(tests: &[TestRun]) -> evscode::R<String> {
	let mut html = String::new();
	for test in tests {
		html += &render_test(test)?;
	}
	Ok(html)
}

fn render_test(test: &TestRun) -> evscode::R<String> {
	Ok(format!(
		r#"
		<tr class="test-row" data-in_path="{in_path}">
			{input}
			{out}
			{desired}
		</tr>
	"#,
		in_path = test.in_path.display(),
		input = render_in_cell(test)?,
		out = render_out_cell(test)?,
		desired = render_desired_cell(test)?
	))
}

fn render_in_cell(test: &TestRun) -> evscode::R<String> {
	Ok(render_cell("", &[ACTION_COPY], &fs::read_to_string(&test.in_path)?, None))
}

fn render_out_cell(test: &TestRun) -> evscode::R<String> {
	use ci::test::Verdict::*;
	let outcome_class = match test.outcome.verdict {
		Accepted => "test-good",
		WrongAnswer | RuntimeError | TimeLimitExceeded => "test-bad",
		IgnoredNoOut => "test-warn",
	};
	let note = match test.outcome.verdict {
		Accepted | WrongAnswer => None,
		RuntimeError => Some("Runtime Error"),
		TimeLimitExceeded => Some("Time Limit Exceeded"),
		IgnoredNoOut => Some("Ignored"),
	};
	Ok(render_cell(outcome_class, &[ACTION_COPY, ACTION_RR], &test.outcome.out, note))
}

fn render_desired_cell(test: &TestRun) -> evscode::R<String> {
	Ok(if test.out_path.exists() {
		render_cell("", &[ACTION_COPY], &fs::read_to_string(&test.out_path)?, None)
	} else {
		render_cell("", &[], "", Some("File does not exist"))
	})
}

struct Action {
	onclick: &'static str,
	icon: &'static str,
}
const ACTION_COPY: Action = Action {
	onclick: "clipcopy()",
	icon: "file_copy",
};
const ACTION_RR: Action = Action {
	onclick: "trigger_rr()",
	icon: "fast_rewind",
};

fn render_cell(class: &str, actions: &[Action], data: &str, note: Option<&str>) -> String {
	let note_div = if let Some(note) = note {
		format!(r#"<div class="test-note">{note}</div>"#, note = note)
	} else {
		format!("")
	};
	let mut action_list = String::new();
	for action in actions {
		action_list += &format!(r#"<div class="test-action material-icons" onclick="{}">{}</div>"#, action.onclick, action.icon);
	}
	format!(
		r#"
		<td style="height: {lines_em}em; line-height: 1.1em;" class="test-cell {class}">
			<div class="test-actions">
				{action_list}
			</div>
			<div class="test-data">
				{data}
			</div>
			{note_div}
		</td>
	"#,
		lines_em = 1.1 * lines(data) as f64,
		class = class,
		action_list = action_list,
		data = html_escape(data.trim()),
		note_div = note_div
	)
}

fn lines(s: &str) -> usize {
	s.trim().chars().filter(|c| char::is_whitespace(*c)).count()
}
fn html_escape(s: &str) -> String {
	s.replace("\n", "<br/>")
}