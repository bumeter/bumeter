//! Đọc frontmatter YAML tối giản của `SKILL.md` (chỉ `name`, `description`).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return v[1..v.len() - 1].replace("\\\"", "\"");
        }
    }
    v.to_string()
}

pub fn parse(text: &str) -> Frontmatter {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Frontmatter::default();
    }
    let block: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    let mut fm = Frontmatter::default();
    let mut i = 0;
    while i < block.len() {
        let line = block[i];
        i += 1;
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        // `key: >` hoặc `key: |`: nối các dòng thụt vào phía sau.
        let value = if value == ">" || value == "|" || value == ">-" || value == "|-" {
            let mut parts = Vec::new();
            while i < block.len() && (block[i].starts_with(' ') || block[i].starts_with('\t') || block[i].trim().is_empty()) {
                if !block[i].trim().is_empty() {
                    parts.push(block[i].trim());
                }
                i += 1;
            }
            parts.join(if value.starts_with('|') { "\n" } else { " " })
        } else {
            unquote(value)
        };
        match key.trim() {
            "name" => fm.name = Some(value),
            "description" => fm.description = Some(value),
            _ => {}
        }
    }
    fm
}
