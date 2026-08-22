//! Project scaffolding (ADR-0009 Stage 3).
//! Tiny template registry that expands to N `files.write_file` under
//! `file:/workspace/projects/<slug>/<date>/` in one `PlanHash`.

use std::collections::HashMap;

pub struct ProjectTemplate {
    pub slug: &'static str,
    pub files: &'static [(&'static str, &'static str)],
}

pub const TEMPLATES: &[ProjectTemplate] = &[
    ProjectTemplate {
        slug: "python-html",
        files: &[
            ("index.html", "<!doctype html><html><head><meta charset=\"utf-8\"><title>Hello</title></head><body><h1>hello world</h1><p>from aios project</p></body></html>"),
            ("server.py", "import http.server, socketserver, pathlib\nPORT=8000\nclass H(http.server.SimpleHTTPRequestHandler):\n    def end_headers(self):\n        self.send_header('Cache-Control','no-cache')\n        super().end_headers()\nif __name__=='__main__':\n    print(f'serving {pathlib.Path.cwd()} at http://127.0.0.1:{PORT}')\n    socketserver.TCPServer(('',PORT), H).serve_forever()\n"),
            ("README.md", "# demo\nscaffolded by aios\\n\\nRun: `python3 server.py` then open http://127.0.0.1:8000"),
        ],
    },
    ProjectTemplate {
        slug: "rust-cli",
        files: &[
            ("Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
            ("src/main.rs", "fn main(){ println!(\"hello from aios rust project\"); }\n"),
            ("README.md", "# demo rust cli\n"),
        ],
    },
];

pub fn find_template(slug: &str) -> Option<&'static ProjectTemplate> {
    TEMPLATES.iter().find(|t| t.slug == slug)
}

pub fn scaffold_files(slug: &str, project: &str, date: &str) -> Option<Vec<(String, String)>> {
    let tmpl = find_template(slug)?;
    Some(tmpl.files.iter().map(|(name, content)| {
        (format!("file:/workspace/projects/{project}/{date}/{name}"), content.to_string())
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaffold_python_html() {
        let files = scaffold_files("python-html", "demo", "2026-08-22").unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0].0.contains("projects/demo/2026-08-22/index.html"));
    }
}
