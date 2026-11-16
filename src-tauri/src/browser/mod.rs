use crate::database::Database;
use crate::settings::BrowserSettings;
use crate::tabs::{Tab, TabState};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct BrowserState {
    pub db: Database,
    pub tabs: Mutex<TabState>,
    pub settings: Mutex<BrowserSettings>,
}

impl BrowserState {
    pub fn new(db: Database, settings: BrowserSettings) -> Arc<Self> {
        let mut tabs = TabState::default();
        let home = settings.homepage.clone();
        let initial = Tab::new(home, false);
        let id = initial.id.clone();
        tabs.tabs.push(initial);
        tabs.active_tab_id = Some(id);
        Arc::new(Self {
            db,
            tabs: Mutex::new(tabs),
            settings: Mutex::new(settings),
        })
    }

    pub fn toggle_tab_pin(&self, tab_id: String) -> Result<(), String> {
        let mut tabs = self.tabs.lock().map_err(|e| format!("Failed to lock tabs: {}", e))?;
        let tab = tabs.tabs.iter_mut().find(|t| t.id == tab_id)
            .ok_or_else(|| format!("Tab not found: {}", tab_id))?;
        tab.toggle_pinned();
        Ok(())
    }

    pub fn toggle_tab_mute(&self, tab_id: String) -> Result<(), String> {
        let mut tabs = self.tabs.lock().map_err(|e| format!("Failed to lock tabs: {}", e))?;
        let tab = tabs.tabs.iter_mut().find(|t| t.id == tab_id)
            .ok_or_else(|| format!("Tab not found: {}", tab_id))?;
        tab.toggle_muted();
        Ok(())
    }
}
