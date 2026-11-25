use iced::theme::{Palette, palette};
use iced::widget::{
    button, column, container, image, row, scrollable, slider, space, stack, text, text_input,
    toggler,
};
use iced::{
    Background, Border, Color, ContentFit, Element, Length, Shadow, Subscription, Task, Theme,
};
use scrive_core::{EolFlavor, SyntaxDef};
use scrive_iced::{CodeEditor, Event};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const RUST_GRAMMAR: &str = include_str!("../assets/rust.sublime-syntax");
const LIGHT_THEME: &str = include_str!("../assets/nexus-light.tmTheme");
const DARK_THEME: &str = include_str!("../assets/scrive-dark.tmTheme");

const WINDOW_BG: Color = Color::from_rgb8(0xF7, 0xF8, 0xFA);
const PANEL_BG: Color = Color::from_rgb8(0xFF, 0xFF, 0xFF);
const BORDER: Color = Color::from_rgb8(0xD9, 0xDE, 0xE5);
const TEXT_MAIN: Color = Color::from_rgb8(0x23, 0x26, 0x29);
const TEXT_MUTED: Color = Color::from_rgb8(0x4D, 0x54, 0x5C);
const HOVER_BG: Color = Color::from_rgb8(0xEE, 0xF1, 0xF5);
const PRESSED_BG: Color = Color::from_rgb8(0xE3, 0xE8, 0xEE);

#[derive(Debug, Clone)]
enum Message {
    Editor(Event),
    NewTab,
    CloseTab(usize),
    SelectTab(usize),
    OpenFile,
    FileOpened(Result<(String, std::path::PathBuf), String>),
    SaveFile,
    SaveAs,
    FileSaved(Result<std::path::PathBuf, String>),
    ToggleSettings,
    SetFontSize(f32),
    SetBgHex(String),
    ApplyBgHex,
    ChooseBgImage,
    BgImageLoaded(Result<(Vec<u8>, PathBuf), String>),
    ClearBgImage,
    ToggleMinimap(bool),
    ImportTheme,
    ThemeImported(Result<String, String>),
    SetThemePreset(bool), // true dark, false light-ish
    OpenRecent(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct EditorConfig {
    theme_text: String,
    bg_hex: String,
    bg_image_path: Option<PathBuf>,
    font_size: f32,
    minimap: bool,
    recent_files: Vec<PathBuf>,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme_text: LIGHT_THEME.to_string(),
            bg_hex: "#FFFFFF".into(),
            bg_image_path: None,
            font_size: 14.0,
            minimap: false,
            recent_files: Vec::new(),
        }
    }
}

fn config_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nexus-code").join("config.json")
}

fn load_config() -> EditorConfig {
    std::fs::read_to_string(config_file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

struct Tab {
    title: String,
    path: Option<std::path::PathBuf>,
    editor: CodeEditor,
}

struct App {
    tabs: Vec<Tab>,
    active: usize,
    theme_text: String,
    font_size: f32,
    minimap: bool,
    bg_hex: String,
    bg_image: Option<iced::widget::image::Handle>,
    bg_image_path: Option<PathBuf>,
    recent_files: Vec<PathBuf>,
    show_settings: bool,
    status: String,
}

impl App {
    fn new() -> Self {
        let config = load_config();
        let theme_text = if scrive_core::TokenTheme::from_tm_theme(&config.theme_text).is_ok() {
            config.theme_text
        } else {
            LIGHT_THEME.to_string()
        };
        let bg_hex = if config.bg_hex.parse::<Color>().is_ok() {
            config.bg_hex
        } else {
            "#FFFFFF".into()
        };
        let font_size = config.font_size.clamp(10.0, 28.0);
        let (bg_image_path, bg_image) = match config
            .bg_image_path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok().map(|data| (path.clone(), data)))
        {
            Some((path, data)) => (
                Some(path),
                Some(iced::widget::image::Handle::from_bytes(data)),
            ),
            None => (None, None),
        };
        let editor_theme = if bg_image.is_some() {
            set_bg_in_theme(&theme_text, "#00000000")
        } else {
            set_bg_in_theme(&theme_text, &bg_hex)
        };
        let editor = make_editor("", &editor_theme, font_size, None);
        Self {
            tabs: vec![Tab {
                title: "sem título".into(),
                path: None,
                editor,
            }],
            active: 0,
            theme_text,
            font_size,
            minimap: config.minimap,
            bg_hex,
            bg_image,
            bg_image_path,
            recent_files: config.recent_files,
            show_settings: false,
            status: "Pronto".into(),
        }
    }

    fn save_config(&self) {
        let config = EditorConfig {
            theme_text: self.theme_text.clone(),
            bg_hex: self.bg_hex.clone(),
            bg_image_path: self.bg_image_path.clone(),
            font_size: self.font_size,
            minimap: self.minimap,
            recent_files: self.recent_files.clone(),
        };
        let path = config_file();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(path, json);
        }
    }

    fn remember_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|recent| recent != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
    }

    fn editor_theme_text(&self) -> String {
        let bg = if self.bg_image.is_some() {
            "#00000000"
        } else {
            &self.bg_hex
        };
        set_bg_in_theme(&self.theme_text, bg)
    }

    fn apply_theme(&mut self) {
        let theme_text = self.editor_theme_text();
        if let Ok(th) = scrive_core::TokenTheme::from_tm_theme(&theme_text) {
            for t in &mut self.tabs {
                t.editor.set_theme(th.clone());
            }
        }
    }

    fn current_grammar(path: Option<&std::path::PathBuf>) -> Option<SyntaxDef> {
        let ext = path
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext.eq_ignore_ascii_case("rs") {
            SyntaxDef::from_sublime_syntax(RUST_GRAMMAR).ok()
        } else {
            None
        }
    }

    fn rebuild_tabs(&mut self) {
        let theme_text = self.editor_theme_text();
        let font_size = self.font_size;
        let mut new_tabs = Vec::new();
        for t in self.tabs.drain(..) {
            let src = t.editor.document().serialize(EolFlavor::Lf);
            let grammar = Self::current_grammar(t.path.as_ref());
            let editor = make_editor(&src, &theme_text, font_size, grammar);
            new_tabs.push(Tab {
                title: t.title,
                path: t.path,
                editor,
            });
        }
        self.tabs = new_tabs;
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(event) => {
                if !self.tabs.is_empty() {
                    return self.tabs[self.active]
                        .editor
                        .update(event)
                        .map(Message::Editor);
                }
                Task::none()
            }
            Message::NewTab => {
                let theme_text = self.editor_theme_text();
                let editor = make_editor("", &theme_text, self.font_size, None);
                self.tabs.push(Tab {
                    title: "sem título".into(),
                    path: None,
                    editor,
                });
                self.active = self.tabs.len() - 1;
                Task::none()
            }
            Message::CloseTab(i) => {
                if self.tabs.len() <= 1 {
                    return Task::none();
                }
                self.tabs.remove(i);
                if self.active >= self.tabs.len() {
                    self.active = self.tabs.len() - 1;
                } else if self.active > i {
                    self.active -= 1;
                }
                Task::none()
            }
            Message::SelectTab(i) => {
                if i < self.tabs.len() {
                    self.active = i;
                }
                Task::none()
            }
            Message::OpenFile => Task::perform(
                async {
                    let file = rfd::AsyncFileDialog::new()
                        .add_filter("Text", &["rs", "txt", "md", "toml", "json"])
                        .pick_file()
                        .await;
                    match file {
                        Some(f) => {
                            let path = f.path().to_path_buf();
                            let data = f.read().await;
                            Ok((String::from_utf8_lossy(&data).to_string(), path))
                        }
                        None => Err("cancelled".to_string()),
                    }
                },
                Message::FileOpened,
            ),
            Message::FileOpened(Ok((text, path))) => {
                let title = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let grammar = Self::current_grammar(Some(&path));
                let theme_text = self.editor_theme_text();
                let editor = make_editor(&text, &theme_text, self.font_size, grammar);
                self.remember_recent(path.clone());
                self.save_config();
                // replace active tab if empty untitled? else new tab
                if self.tabs.len() == 1 && self.tabs[0].path.is_none() {
                    let src_check = self.tabs[0].editor.document().serialize(EolFlavor::Lf);
                    if src_check.trim().is_empty() {
                        self.tabs[0] = Tab {
                            title,
                            path: Some(path),
                            editor,
                        };
                        return Task::none();
                    }
                }
                self.tabs.push(Tab {
                    title,
                    path: Some(path),
                    editor,
                });
                self.active = self.tabs.len() - 1;
                self.status = "Arquivo aberto".into();
                Task::none()
            }
            Message::FileOpened(Err(e)) => {
                if e != "cancelled" {
                    self.status = format!("Falha ao abrir: {}", e);
                }
                Task::none()
            }
            Message::OpenRecent(path) => Task::perform(
                async move {
                    std::fs::read(&path)
                        .map(|data| (String::from_utf8_lossy(&data).to_string(), path.clone()))
                        .map_err(|e| format!("{}: {}", path.display(), e))
                },
                Message::FileOpened,
            ),
            Message::SaveFile => {
                if self.tabs.is_empty() {
                    return Task::none();
                }
                if let Some(path) = self.tabs[self.active].path.clone() {
                    let text = self.tabs[self.active]
                        .editor
                        .document()
                        .serialize(EolFlavor::Lf);
                    Task::perform(
                        async move {
                            std::fs::write(&path, text)
                                .map(|_| path)
                                .map_err(|e| e.to_string())
                        },
                        Message::FileSaved,
                    )
                } else {
                    // save as
                    self.update(Message::SaveAs)
                }
            }
            Message::SaveAs => {
                let text = if !self.tabs.is_empty() {
                    self.tabs[self.active]
                        .editor
                        .document()
                        .serialize(EolFlavor::Lf)
                } else {
                    String::new()
                };
                Task::perform(
                    async move {
                        let file = rfd::AsyncFileDialog::new().save_file().await;
                        match file {
                            Some(f) => {
                                let path = f.path().to_path_buf();
                                std::fs::write(&path, text).map_err(|e| e.to_string())?;
                                Ok(path)
                            }
                            None => Err("cancelled".to_string()),
                        }
                    },
                    Message::FileSaved,
                )
            }
            Message::FileSaved(Ok(path)) => {
                if !self.tabs.is_empty() {
                    self.tabs[self.active].path = Some(path.clone());
                    self.tabs[self.active].title = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                }
                self.remember_recent(path);
                self.save_config();
                self.status = "Salvo".into();
                Task::none()
            }
            Message::FileSaved(Err(e)) => {
                if e != "cancelled" {
                    self.status = format!("Falha ao salvar: {}", e);
                }
                Task::none()
            }
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                Task::none()
            }
            Message::SetFontSize(size) => {
                self.font_size = size;
                self.rebuild_tabs();
                self.save_config();
                Task::none()
            }
            Message::SetBgHex(v) => {
                self.bg_hex = v;
                Task::none()
            }
            Message::ApplyBgHex => {
                if self.bg_hex.parse::<Color>().is_err() {
                    self.status = "Cor inválida. Use #RRGGBB.".into();
                    return Task::none();
                }
                // Keep a wallpaper active when the color field is applied; the
                // color is saved for when the image is removed explicitly.
                self.apply_theme();
                self.save_config();
                self.status = if self.bg_image.is_some() {
                    "Imagem mantida; cor de fundo salva".into()
                } else {
                    "Fundo aplicado".into()
                };
                Task::none()
            }
            Message::ChooseBgImage => Task::perform(
                async {
                    let file = rfd::AsyncFileDialog::new()
                        .add_filter("Imagem", &["png", "jpg", "jpeg", "bmp"])
                        .pick_file()
                        .await;
                    match file {
                        Some(f) => {
                            let path = f.path().to_path_buf();
                            let data = f.read().await;
                            Ok((data, path))
                        }
                        None => Err("cancelled".to_string()),
                    }
                },
                Message::BgImageLoaded,
            ),
            Message::BgImageLoaded(Ok((data, path))) => {
                self.bg_image = Some(iced::widget::image::Handle::from_bytes(data));
                self.bg_image_path = Some(path);
                self.apply_theme();
                self.save_config();
                self.status = "Imagem de fundo aplicada".into();
                Task::none()
            }
            Message::BgImageLoaded(Err(e)) => {
                if e != "cancelled" {
                    self.status = format!("Falha ao carregar imagem: {}", e);
                }
                Task::none()
            }
            Message::ClearBgImage => {
                self.bg_image = None;
                self.bg_image_path = None;
                self.apply_theme();
                self.save_config();
                self.status = "Imagem removida".into();
                Task::none()
            }
            Message::ToggleMinimap(v) => {
                self.minimap = v;
                self.save_config();
                Task::none()
            }
            Message::ImportTheme => Task::perform(
                async {
                    let file = rfd::AsyncFileDialog::new()
                        .add_filter("Tema", &["tmTheme"])
                        .pick_file()
                        .await;
                    match file {
                        Some(f) => {
                            let data = f.read().await;
                            Ok(String::from_utf8_lossy(&data).to_string())
                        }
                        None => Err("cancelled".to_string()),
                    }
                },
                Message::ThemeImported,
            ),
            Message::ThemeImported(Ok(text)) => {
                match scrive_core::TokenTheme::from_tm_theme(&text) {
                    Ok(_) => {
                        self.theme_text = text;
                        self.apply_theme();
                        self.save_config();
                        self.status = "Tema importado".into();
                    }
                    Err(e) => self.status = format!("Erro ao ler tema: {}", e),
                }
                Task::none()
            }
            Message::ThemeImported(Err(e)) => {
                if e != "cancelled" {
                    self.status = format!("Falha ao importar: {}", e);
                }
                Task::none()
            }
            Message::SetThemePreset(dark) => {
                self.theme_text = if dark {
                    self.bg_hex = "#1C1E24".into();
                    DARK_THEME.to_string()
                } else {
                    self.bg_hex = "#FFFFFF".into();
                    LIGHT_THEME.to_string()
                };
                self.apply_theme();
                self.save_config();
                self.status = if dark { "Tema escuro" } else { "Tema claro" }.into();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let wallpaper = self.bg_image.is_some();
        let settings_label = if self.show_settings {
            "Fechar"
        } else {
            "Configurações"
        };
        let toolbar = container(
            row![
                text("Nexus Code").size(14).color(TEXT_MAIN),
                space::horizontal(),
                nav_button("Novo", Message::NewTab),
                nav_button("Abrir", Message::OpenFile),
                nav_button("Salvar", Message::SaveFile),
                nav_button("Salvar como", Message::SaveAs),
                nav_button(settings_label, Message::ToggleSettings),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding([5, 10])
        .style(panel_style(wallpaper));

        let mut tabs_row = row![].spacing(3).align_y(iced::Alignment::Center);
        for (i, t) in self.tabs.iter().enumerate() {
            let active = i == self.active;
            let title = t.title.clone();
            let tab =
                button(
                    text(title)
                        .size(12)
                        .color(if active { TEXT_MAIN } else { TEXT_MUTED }),
                )
                .padding([5, 10])
                .style(move |theme, status| tab_style(theme, status, active, wallpaper))
                .on_press(Message::SelectTab(i));
            let close = button(text("×").size(14).color(TEXT_MUTED))
                .padding([1, 5])
                .style(icon_button_style)
                .on_press(Message::CloseTab(i));
            tabs_row = tabs_row.push(row![tab, close].spacing(0).align_y(iced::Alignment::Center));
        }

        let tabs_bar = container(tabs_row)
            .width(Length::Fill)
            .padding([4, 8])
            .style(panel_style(wallpaper));

        let editor_view: Element<Message> = if self.tabs.is_empty() {
            container(text("Nenhuma aba aberta").size(13).color(TEXT_MUTED))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else {
            self.tabs[self.active].editor.view().map(Message::Editor)
        };

        let main_area: Element<Message> = if self.minimap && !self.tabs.is_empty() {
            let src = self.tabs[self.active]
                .editor
                .document()
                .serialize(EolFlavor::Lf);
            let minimap = container(
                scrollable(
                    text(src)
                        .size(4)
                        .font(iced::Font::MONOSPACE)
                        .color(TEXT_MUTED),
                )
                .height(Length::Fill),
            )
            .width(Length::Fixed(150.0))
            .height(Length::Fill)
            .padding(4)
            .style(|_| container::Style {
                background: Some(Color::from_rgb8(0xFB, 0xFC, 0xFD).into()),
                ..Default::default()
            });
            row![editor_view, minimap].spacing(0).into()
        } else {
            editor_view
        };

        let bg_color = self.bg_hex.parse::<Color>().unwrap_or(Color::WHITE);
        let editor_area = container(main_area)
            .width(Length::Fill)
            .height(Length::Fill);

        let mut content = column![toolbar, separator(), tabs_bar, separator(), editor_area,]
            .width(Length::Fill)
            .height(Length::Fill);

        if self.show_settings {
            let mut recent = column![text("Recentes").size(13).color(TEXT_MUTED)].spacing(4);
            if self.recent_files.is_empty() {
                recent = recent.push(text("Nenhum arquivo recente").size(12).color(TEXT_MUTED));
            } else {
                for path in self.recent_files.iter().take(5) {
                    let label = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    recent = recent.push(outlined_button(label, Message::OpenRecent(path.clone())));
                }
            }

            let settings = container(
                column![
                    text("Configurações").size(15).color(TEXT_MAIN),
                    row![
                        text("Tamanho da fonte").size(13).color(TEXT_MUTED),
                        slider(10.0..=28.0, self.font_size, Message::SetFontSize).width(180),
                        text(format!("{:.0} px", self.font_size))
                            .size(12)
                            .color(TEXT_MUTED),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    row![
                        text("Cor de fundo").size(13).color(TEXT_MUTED),
                        text_input("#FFFFFF", &self.bg_hex)
                            .on_input(Message::SetBgHex)
                            .on_submit(Message::ApplyBgHex)
                            .style(neutral_text_input)
                            .width(140),
                        outlined_button("Aplicar cor", Message::ApplyBgHex),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    row![
                        outlined_button("Escolher imagem", Message::ChooseBgImage),
                        outlined_button("Remover imagem", Message::ClearBgImage),
                    ]
                    .spacing(6),
                    row![
                        text("Minimapa").size(13).color(TEXT_MUTED),
                        toggler(self.minimap).on_toggle(Message::ToggleMinimap),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    row![
                        outlined_button("Importar tema (.tmTheme)", Message::ImportTheme),
                        outlined_button("Tema claro", Message::SetThemePreset(false)),
                        outlined_button("Tema escuro", Message::SetThemePreset(true)),
                    ]
                    .spacing(6),
                    recent,
                ]
                .spacing(10),
            )
            .width(Length::Fill)
            .padding(12)
            .style(settings_style(wallpaper));
            content = content.push(separator()).push(settings);
        }

        let status_bar = container(
            row![
                text(&self.status).size(12).color(TEXT_MUTED),
                space::horizontal(),
                text("UTF-8   LF").size(12).color(TEXT_MUTED),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding([4, 10])
        .style(panel_style(wallpaper));

        content = content.push(separator()).push(status_bar);

        let body = container(content).width(Length::Fill).height(Length::Fill);

        if let Some(handle) = &self.bg_image {
            stack![
                image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(ContentFit::Cover),
                body,
            ]
            .into()
        } else {
            body.style(move |_| container::Style {
                background: Some(bg_color.into()),
                ..Default::default()
            })
            .into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.tabs.is_empty() {
            Subscription::none()
        } else {
            self.tabs[self.active]
                .editor
                .subscription()
                .map(Message::Editor)
        }
    }
}

fn nav_button(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(13))
        .padding([5, 10])
        .style(flat_button)
        .on_press(message)
        .into()
}

fn outlined_button(label: impl Into<String>, message: Message) -> Element<'static, Message> {
    button(text(label.into()).size(12))
        .padding([4, 10])
        .style(outlined_button_style)
        .on_press(message)
        .into()
}

fn separator() -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|_| container::Style {
            background: Some(BORDER.into()),
            ..Default::default()
        })
        .into()
}

fn flat_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Active => (None, TEXT_MAIN),
        button::Status::Hovered => (Some(HOVER_BG), TEXT_MAIN),
        button::Status::Pressed => (Some(PRESSED_BG), TEXT_MAIN),
        button::Status::Disabled => (None, Color::from_rgb8(0xA8, 0xAE, 0xB6)),
    };
    button::Style {
        background: background.map(Background::Color),
        text_color,
        border: Border::default().rounded(5.0),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn outlined_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Active => (PANEL_BG, TEXT_MAIN),
        button::Status::Hovered => (Color::from_rgb8(0xF4, 0xF6, 0xF9), TEXT_MAIN),
        button::Status::Pressed => (Color::from_rgb8(0xE9, 0xED, 0xF2), TEXT_MAIN),
        button::Status::Disabled => (PANEL_BG, Color::from_rgb8(0xA8, 0xAE, 0xB6)),
    };
    button::Style {
        background: Some(background.into()),
        text_color,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 5.0.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

fn icon_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Active => None,
        button::Status::Hovered => Some(Color::from_rgb8(0xF0, 0xE2, 0xE2)),
        button::Status::Pressed => Some(Color::from_rgb8(0xE8, 0xD5, 0xD5)),
        button::Status::Disabled => None,
    };
    button::Style {
        background: background.map(Background::Color),
        text_color: TEXT_MUTED,
        border: Border::default().rounded(4.0),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn tab_style(
    _theme: &Theme,
    status: button::Status,
    active: bool,
    wallpaper: bool,
) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let active_bg = if wallpaper {
        Color::from_rgba8(255, 255, 255, 0.78)
    } else {
        PANEL_BG
    };
    let hover_bg = if wallpaper {
        Color::from_rgba8(255, 255, 255, 0.55)
    } else {
        HOVER_BG
    };
    let (background, text_color, border) = if active {
        (
            active_bg,
            TEXT_MAIN,
            Border {
                color: BORDER,
                width: 1.0,
                radius: iced::border::Radius {
                    top_left: 6.0,
                    top_right: 6.0,
                    ..Default::default()
                },
            },
        )
    } else if hovered {
        (hover_bg, TEXT_MAIN, Border::default().rounded(6.0))
    } else {
        (
            Color::TRANSPARENT,
            TEXT_MUTED,
            Border::default().rounded(6.0),
        )
    };
    button::Style {
        background: Some(background.into()),
        text_color,
        border,
        shadow: Shadow::default(),
        snap: true,
    }
}

fn panel_style(wallpaper: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: if wallpaper {
            Some(Color::from_rgba8(255, 255, 255, 0.42).into())
        } else {
            Some(PANEL_BG.into())
        },
        ..Default::default()
    }
}

fn settings_style(wallpaper: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: if wallpaper {
            Some(Color::from_rgba8(255, 255, 255, 0.52).into())
        } else {
            Some(Color::from_rgb8(0xF7, 0xF8, 0xFA).into())
        },
        ..Default::default()
    }
}

fn neutral_text_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: Color::WHITE.into(),
        border: Border {
            color: if focused {
                Color::from_rgb8(0x8C, 0x96, 0xA1)
            } else {
                BORDER
            },
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: Color::from_rgb8(0x9A, 0xA1, 0xA9),
        value: TEXT_MAIN,
        selection: Color::from_rgba8(0x6B, 0x9E, 0xC9, 0.35),
    }
}

fn nexus_theme(state: &App) -> Theme {
    let base_palette = Palette {
        background: WINDOW_BG,
        text: TEXT_MAIN,
        primary: Color::from_rgb8(0x6B, 0x72, 0x80),
        success: Color::from_rgb8(0x2E, 0x7D, 0x32),
        warning: Color::from_rgb8(0xB2, 0x6A, 0x00),
        danger: Color::from_rgb8(0xC6, 0x28, 0x28),
    };
    let transparent_editor = state.bg_image.is_some();

    Theme::custom_with_fn("Nexus Code", base_palette, move |base| {
        let mut extended = palette::Extended::generate(base);
        if transparent_editor {
            // The scrive widget paints its viewport with the iced background
            // color, not the TextMate theme background. Make only that base
            // color transparent so the wallpaper in the stack behind the editor
            // shows through, while the rest of the UI keeps its solid chrome.
            extended.background.base = palette::Pair {
                color: Color::from_rgba8(255, 255, 255, 0.30),
                text: TEXT_MAIN,
            };
        }
        extended
    })
}

fn make_editor(
    source: &str,
    theme_text: &str,
    font_size: f32,
    grammar: Option<SyntaxDef>,
) -> CodeEditor {
    let theme = scrive_core::TokenTheme::from_tm_theme(theme_text)
        .unwrap_or_else(|_| scrive_iced::scrive_dark_theme());
    let mut ed = CodeEditor::new(source).theme(theme).text_size(font_size);
    if let Some(g) = grammar {
        ed = ed.language(g);
    }
    ed
}

fn set_bg_in_theme(theme_text: &str, bg_hex: &str) -> String {
    // replace first background string
    if let Some(start) = theme_text.find("<key>background</key><string>") {
        let value_start = start + "<key>background</key><string>".len();
        if let Some(end) = theme_text[value_start..].find("</string>") {
            let end = value_start + end;
            let mut s = String::new();
            s.push_str(&theme_text[..value_start]);
            s.push_str(bg_hex);
            s.push_str(&theme_text[end..]);
            return s;
        }
    }
    theme_text.to_string()
}

fn main() -> iced::Result {
    let window_settings = iced::window::Settings {
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "nexus-code".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let app = iced::application(App::new, App::update, App::view)
        .title("Nexus Code")
        .theme(nexus_theme)
        .subscription(App::subscription)
        .window(window_settings);
    scrive_iced::required_fonts()
        .iter()
        .fold(app, |app, font| app.font(*font))
        .run()
}
