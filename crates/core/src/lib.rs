pub fn parse_events(text: &str) -> Vec<markdown::event::Event> {
    let opts = markdown::ParseOptions::gfm();
    markdown::parser::parse(text, &opts).expect("parse").0
}
