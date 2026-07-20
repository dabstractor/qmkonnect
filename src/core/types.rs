#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub app_class: String,
    pub title: String,
}

impl WindowInfo {
    pub fn new(app_class: String, title: String) -> Self {
        Self { app_class, title }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_info_creation() {
        let app_class = "TestApp".to_string();
        let title = "Test Title".to_string();

        let window_info = WindowInfo::new(app_class.clone(), title.clone());

        assert_eq!(window_info.app_class, app_class);
        assert_eq!(window_info.title, title);
    }

    #[test]
    fn test_window_info_equality() {
        let window1 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let window2 = WindowInfo::new("App1".to_string(), "Title1".to_string());
        let window3 = WindowInfo::new("App2".to_string(), "Title1".to_string());

        assert_eq!(window1, window2);
        assert_ne!(window1, window3);
    }
}
