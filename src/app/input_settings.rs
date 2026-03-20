use crossterm::event::{self, KeyCode};

use crate::error::Error;

use super::*;

/// Index of the Discord App ID field in the settings page.
/// Update this if new fields are inserted before it.
const DISCORD_FIELD: usize = 3;

impl App {
    /// Handle settings page keys
    pub(super) async fn handle_settings_key(&mut self, key: event::KeyEvent) -> Result<(), Error> {
        let mut config_changed = false;

        {
            let mut state = self.state.write().await;
            let field = state.settings_state.selected_field;

            match key.code {
                // Navigate between fields
                KeyCode::Up | KeyCode::Char('k') => {
                    if field > 0 {
                        state.settings_state.selected_field = field - 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') if field < DISCORD_FIELD => {
                    state.settings_state.selected_field = field + 1;
                }

                // Text input for Discord App ID (field 3)
                KeyCode::Char(c) if field == DISCORD_FIELD => {
                    if c.is_ascii_digit() {
                        state.settings_state.discord_app_id_input.push(c);
                    }
                }
                KeyCode::Backspace if field == DISCORD_FIELD => {
                    state.settings_state.discord_app_id_input.pop();
                }
                KeyCode::Enter if field == DISCORD_FIELD => {
                    let input = state.settings_state.discord_app_id_input.trim().to_string();
                    let new_id: u64 = input.parse().unwrap_or(0);
                    state.config.discord_app_id = new_id;
                    config_changed = true;
                }

                // Left — applies to selector fields (0-2)
                KeyCode::Left | KeyCode::Char('h') => match field {
                    0 => {
                        state.settings_state.prev_theme();
                        state.config.theme = state.settings_state.theme_name().to_string();
                        let label = state.settings_state.theme_name().to_string();
                        state.notify(format!("Theme: {}", label));
                        config_changed = true;
                    }
                    1 if state.cava_available => {
                        state.settings_state.cava_enabled = !state.settings_state.cava_enabled;
                        state.config.cava = state.settings_state.cava_enabled;
                        let status = if state.settings_state.cava_enabled { "On" } else { "Off" };
                        state.notify(format!("Cava: {}", status));
                        config_changed = true;
                    }
                    2 if state.cava_available => {
                        let cur = state.settings_state.cava_size;
                        if cur > 10 {
                            let new_size = cur - 5;
                            state.settings_state.cava_size = new_size;
                            state.config.cava_size = new_size;
                            state.notify(format!("Cava Size: {}%", new_size));
                            config_changed = true;
                        }
                    }
                    _ => {}
                },

                // Right / Enter / Space — applies to selector fields (0-2)
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                    match field {
                        0 => {
                            state.settings_state.next_theme();
                            state.config.theme = state.settings_state.theme_name().to_string();
                            let label = state.settings_state.theme_name().to_string();
                            state.notify(format!("Theme: {}", label));
                            config_changed = true;
                        }
                        1 if state.cava_available => {
                            state.settings_state.cava_enabled = !state.settings_state.cava_enabled;
                            state.config.cava = state.settings_state.cava_enabled;
                            let status = if state.settings_state.cava_enabled { "On" } else { "Off" };
                            state.notify(format!("Cava: {}", status));
                            config_changed = true;
                        }
                        2 if state.cava_available => {
                            let cur = state.settings_state.cava_size;
                            if cur < 80 {
                                let new_size = cur + 5;
                                state.settings_state.cava_size = new_size;
                                state.config.cava_size = new_size;
                                state.notify(format!("Cava Size: {}%", new_size));
                                config_changed = true;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if config_changed {
            let state = self.state.read().await;
            let new_discord_id = state.config.discord_app_id;
            let field = state.settings_state.selected_field;

            if let Err(e) = state.config.save_to_default_path() {
                drop(state);
                let mut state = self.state.write().await;
                state.notify_error(format!("Failed to save: {}", e));
            } else {
                // Start/stop cava based on new setting, or restart on theme change
                let cava_enabled = state.settings_state.cava_enabled;
                let td = state.settings_state.current_theme();
                let g = td.cava_gradient.clone();
                let h = td.cava_horizontal_gradient.clone();
                let cs = state.settings_state.cava_size as u32;
                let cava_running = self.cava_parser.is_some();
                drop(state);

                if cava_enabled {
                    self.start_cava(&g, &h, cs);
                } else if cava_running {
                    self.stop_cava();
                    let mut state = self.state.write().await;
                    state.cava_screen.clear();
                }

                // Start/restart Discord thread if the app ID changed
                if field == DISCORD_FIELD {
                    self.discord_tx = None; // drop old sender → old thread exits
                    if new_discord_id != 0 {
                        self.discord_tx = Some(crate::discord::start_discord_thread(new_discord_id));
                        let mut state = self.state.write().await;
                        state.notify(format!("Discord App ID saved ({})", new_discord_id));
                    } else {
                        let mut state = self.state.write().await;
                        state.notify("Discord Rich Presence disabled");
                    }
                }
            }
        }

        Ok(())
    }
}
