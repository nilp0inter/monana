use crate::metadata::context::MediaContext;
use crate::template::apply_template;
use anyhow::Result;
use rhai::{Dynamic, Engine, Scope};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub rulesets: Vec<Ruleset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ruleset {
    pub name: String,
    pub input: InputSpec,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputSpec {
    #[serde(deserialize_with = "deserialize_cmdline")]
    Cmdline,
    Prefixed(String),
}

fn deserialize_cmdline<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "cmdline" {
        Ok(())
    } else {
        Err(serde::de::Error::custom("expected 'cmdline'"))
    }
}

impl InputSpec {
    pub fn parse_type(&self) -> (&str, Option<&str>) {
        match self {
            InputSpec::Cmdline => ("cmdline", None),
            InputSpec::Prefixed(s) => {
                if let Some((prefix, value)) = s.split_once(':') {
                    (prefix, Some(value))
                } else {
                    ("unknown", None)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub condition: String,
    pub template: String,
    pub action: ActionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionSpec {
    #[serde(rename = "move")]
    Move,
    #[serde(rename = "copy")]
    Copy,
    #[serde(rename = "symlink")]
    Symlink,
    #[serde(rename = "hardlink")]
    Hardlink,
    #[serde(untagged)]
    Command(String),
}

impl ActionSpec {
    pub fn parse_action(&self) -> (&str, Option<&str>) {
        match self {
            ActionSpec::Move => ("move", None),
            ActionSpec::Copy => ("copy", None),
            ActionSpec::Symlink => ("symlink", None),
            ActionSpec::Hardlink => ("hardlink", None),
            ActionSpec::Command(s) => {
                if let Some((prefix, cmd)) = s.split_once(':') {
                    if prefix == "cmd" {
                        ("cmd", Some(cmd))
                    } else {
                        ("unknown", None)
                    }
                } else {
                    ("unknown", None)
                }
            }
        }
    }
}

pub struct RuleEngine {
    engine: Engine,
}

impl RuleEngine {
    pub fn new() -> Result<Self> {
        let mut engine = Engine::new();

        // Configure for expression-only evaluation
        engine.set_max_expr_depths(64, 64);

        Ok(Self { engine })
    }

    pub fn evaluate_condition(&self, condition: &str, context: &MediaContext) -> Result<bool> {
        let mut scope = Scope::new();

        // Add time variables
        let time = &context.time;
        scope.push("time_yyyy", time.yyyy.clone());
        scope.push("time_mm", time.mm.clone());
        scope.push("time_dd", time.dd.clone());
        scope.push("time_month_name", time.month_name.clone());
        scope.push("time_weekday", time.weekday.clone());
        scope.push("time_hh", time.hh.clone());
        scope.push("time_min", time.min.clone());
        scope.push("time_ss", time.ss.clone());

        // Add space variables
        let space = &context.space;
        scope.push("space_country", space.country.clone());
        scope.push("space_country_code", space.country_code.clone());
        scope.push("space_state", space.state.clone());
        scope.push("space_city", space.city.clone());
        scope.push("space_district", space.district.clone());
        scope.push("space_road", space.road.clone());
        scope.push("space_lat", space.lat);
        scope.push("space_lon", space.lon);
        if let Some(altitude) = space.altitude {
            scope.push("space_altitude", altitude);
        }

        // Add source variables
        let source = &context.source;
        scope.push("source_path", source.path.clone());
        scope.push("source_name", source.name.clone());
        scope.push("source_extension", source.extension.clone());
        scope.push("source_original", source.original.clone());
        scope.push("source_size", source.size as i64);

        // Add media variables
        let media = &context.media;
        scope.push("media_type", media.r#type.clone());
        scope.push("media_width", media.width as i64);
        scope.push("media_height", media.height as i64);
        if let Some(duration) = media.duration {
            scope.push("media_duration", duration);
        } else {
            scope.push("media_duration", 0.0);
        }
        if let Some(camera_make) = &media.camera_make {
            scope.push("media_camera_make", camera_make.clone());
        } else {
            scope.push("media_camera_make", "".to_string());
        }
        if let Some(camera_model) = &media.camera_model {
            scope.push("media_camera_model", camera_model.clone());
        } else {
            scope.push("media_camera_model", "".to_string());
        }
        if let Some(orientation) = media.orientation {
            scope.push("media_orientation", orientation as i64);
        }

        // Evaluate the condition expression
        let result: Dynamic = self
            .engine
            .eval_expression_with_scope(&mut scope, condition)
            .map_err(|e| anyhow::anyhow!("Failed to evaluate condition: {}", e))?;

        // Convert to boolean
        Ok(result.as_bool().unwrap_or(false))
    }

    pub fn process_rule(
        &self,
        rule: &Rule,
        context: &MediaContext,
    ) -> Result<Option<(String, ActionSpec)>> {
        if self.evaluate_condition(&rule.condition, context)? {
            // Apply template to get the destination path
            let destination = apply_template(&rule.template, context)?;
            Ok(Some((destination.to_string(), rule.action.clone())))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::context::{MediaInfo, TimeContext};

    #[test]
    fn test_simple_condition() {
        let engine = RuleEngine::new().unwrap();
        let defaultctx = MediaContext::default();
        let context = MediaContext {
            time: TimeContext {
                yyyy: "2024".to_string(),
                mm: "12".to_string(),
                dd: "25".to_string(),
                hh: "14".to_string(),
                min: "30".to_string(),
                ss: "45".to_string(),
                month_name: "December".to_string(),
                weekday: "Monday".to_string(),
                timestamp: None,
            },
            media: defaultctx.media.clone(),
            source: defaultctx.source.clone(),
            space: defaultctx.space.clone(),
        };

        assert!(
            engine
                .evaluate_condition("time_yyyy == \"2024\"", &context)
                .unwrap()
        );
        assert!(
            !engine
                .evaluate_condition("time_yyyy == \"2023\"", &context)
                .unwrap()
        );
        assert!(
            engine
                .evaluate_condition("time_mm == \"12\"", &context)
                .unwrap()
        );
    }

    #[test]
    fn test_complex_condition() {
        let engine = RuleEngine::new().unwrap();
        let ctxdefault = MediaContext::default();

        let timectx = TimeContext {
            yyyy: "2024".to_string(),
            mm: "07".to_string(),
            dd: "04".to_string(),
            hh: "16".to_string(),
            min: "00".to_string(),
            ss: "00".to_string(),
            month_name: "July".to_string(),
            weekday: "Thursday".to_string(),
            timestamp: None,
        };

        let mediactx = MediaInfo {
            r#type: "image".to_string(),
            width: 1920,
            height: 1080,
            duration: None,
            camera_make: None,
            camera_model: None,
            orientation: None,
        };

        let context = MediaContext {
            time: timectx,
            media: mediactx,
            source: ctxdefault.source.clone(),
            space: ctxdefault.space.clone(),
        };

        let condition =
            "time_yyyy == \"2024\" && time_month_name == \"July\" && media_width > 1000";
        assert!(engine.evaluate_condition(condition, &context).unwrap());
    }

    #[test]
    fn test_process_rule() {
        let engine = RuleEngine::new().unwrap();
        let mut context = MediaContext::default();

        context.time = TimeContext {
            yyyy: "2024".to_string(),
            mm: "12".to_string(),
            dd: "25".to_string(),
            hh: "14".to_string(),
            min: "30".to_string(),
            ss: "45".to_string(),
            month_name: "December".to_string(),
            weekday: "Monday".to_string(),
            timestamp: None,
        };

        context.source = crate::metadata::context::SourceContext {
            path: "/tmp/photo.jpg".to_string(),
            name: "photo".to_string(),
            extension: "jpg".to_string(),
            original: "photo.jpg".to_string(),
            size: 1024,
        };

        context.media = MediaInfo {
            r#type: "image".to_string(),
            width: 1920,
            height: 1080,
            duration: None,
            camera_make: None,
            camera_model: None,
            orientation: None,
        };

        let rule = Rule {
            condition: "media_type == \"image\"".to_string(),
            template: "{time.yyyy}/{time.mm}/{source.name}.{source.extension}".to_string(),
            action: ActionSpec::Move,
        };

        let result = engine.process_rule(&rule, &context).unwrap();
        assert!(result.is_some());

        let (destination, action) = result.unwrap();
        assert_eq!(destination, "2024/12/photo.jpg");
        assert!(matches!(action, ActionSpec::Move));
    }

    #[test]
    fn test_rule_condition_not_met() {
        let engine = RuleEngine::new().unwrap();
        let defaultctx = MediaContext::default();

        let mediactx = MediaInfo {
            r#type: "video".to_string(),
            width: 1920,
            height: 1080,
            duration: Some(120.5),
            camera_make: None,
            camera_model: None,
            orientation: None,
        };

        let context = MediaContext {
            media: mediactx,
            time: defaultctx.time.clone(),
            source: defaultctx.source.clone(),
            space: defaultctx.space.clone(),
        };

        let rule = Rule {
            condition: "media_type == \"image\"".to_string(),
            template: "{time.yyyy}/{time.mm}/{source.name}.{source.extension}".to_string(),
            action: ActionSpec::Copy,
        };

        let result = engine.process_rule(&rule, &context).unwrap();
        assert!(result.is_none());
    }
}
