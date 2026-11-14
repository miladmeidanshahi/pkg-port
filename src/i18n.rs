#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Language {
    #[default]
    English,
    Persian,
}

impl Language {
    pub fn is_rtl(self) -> bool {
        self == Self::Persian
    }

    pub fn text(self, key: &'static str) -> String {
        rust_i18n::t!(key, locale = if self.is_rtl() { "fa" } else { "en" }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn english_is_default_and_locales_resolve() {
        assert_eq!(Language::default(), Language::English);
        assert_eq!(Language::English.text("app.proxy"), "Proxy");
        assert_eq!(Language::Persian.text("app.proxy"), "پروکسی");
    }
}
