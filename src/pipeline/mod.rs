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

        // Create time object map
        let mut time_map = rhai::Map::new();
        let time = &context.time;
        time_map.insert("yyyy".into(), Dynamic::from(time.yyyy.clone()));
        time_map.insert("mm".into(), Dynamic::from(time.mm.clone()));
        time_map.insert("dd".into(), Dynamic::from(time.dd.clone()));
        time_map.insert("month_name".into(), Dynamic::from(time.month_name.clone()));
        time_map.insert("weekday".into(), Dynamic::from(time.weekday.clone()));
        time_map.insert("hh".into(), Dynamic::from(time.hh.clone()));
        time_map.insert("min".into(), Dynamic::from(time.min.clone()));
        time_map.insert("ss".into(), Dynamic::from(time.ss.clone()));
        scope.push("time", time_map);

        // Create space object map
        let mut space_map = rhai::Map::new();
        let space = &context.space;
        space_map.insert("country".into(), Dynamic::from(space.country.clone()));
        space_map.insert(
            "country_code".into(),
            Dynamic::from(space.country_code.clone()),
        );
        space_map.insert("state".into(), Dynamic::from(space.state.clone()));
        space_map.insert("city".into(), Dynamic::from(space.city.clone()));
        space_map.insert("district".into(), Dynamic::from(space.district.clone()));
        space_map.insert("road".into(), Dynamic::from(space.road.clone()));
        space_map.insert("lat".into(), Dynamic::from(space.lat));
        space_map.insert("lon".into(), Dynamic::from(space.lon));
        if let Some(altitude) = space.altitude {
            space_map.insert("altitude".into(), Dynamic::from(altitude));
        }
        scope.push("space", space_map);

        // Create source object map
        let mut source_map = rhai::Map::new();
        let source = &context.source;
        source_map.insert("path".into(), Dynamic::from(source.path.clone()));
        source_map.insert("name".into(), Dynamic::from(source.name.clone()));
        source_map.insert("extension".into(), Dynamic::from(source.extension.clone()));
        source_map.insert("original".into(), Dynamic::from(source.original.clone()));
        source_map.insert("size".into(), Dynamic::from(source.size as i64));
        scope.push("source", source_map);

        // Add type variable
        scope.push("type", context.r#type.clone());

        // Add meta as a Rhai object map
        let mut meta_map = rhai::Map::new();
        for (key, value) in &context.meta {
            meta_map.insert(key.clone().into(), value.clone());
        }
        scope.push("meta", meta_map);

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
    use crate::metadata::context::TimeContext;

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
            r#type: defaultctx.r#type.clone(),
            meta: defaultctx.meta.clone(),
            source: defaultctx.source.clone(),
            space: defaultctx.space.clone(),
        };

        assert!(
            engine
                .evaluate_condition("time.yyyy == \"2024\"", &context)
                .unwrap()
        );
        assert!(
            !engine
                .evaluate_condition("time.yyyy == \"2023\"", &context)
                .unwrap()
        );
        assert!(
            engine
                .evaluate_condition("time.mm == \"12\"", &context)
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

        let mut context = MediaContext {
            time: timectx,
            r#type: "image".to_string(),
            source: ctxdefault.source.clone(),
            space: ctxdefault.space.clone(),
            meta: Default::default(),
        };

        // Add metadata values
        context
            .meta
            .insert("ImageWidth".to_string(), Dynamic::from(1920i64));
        context
            .meta
            .insert("ImageHeight".to_string(), Dynamic::from(1080i64));

        let condition =
            "time.yyyy == \"2024\" && time.month_name == \"July\" && meta.ImageWidth > 1000";
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

        context.r#type = "image".to_string();
        context
            .meta
            .insert("ImageWidth".to_string(), Dynamic::from(1920i64));
        context
            .meta
            .insert("ImageHeight".to_string(), Dynamic::from(1080i64));

        let rule = Rule {
            condition: "type == \"image\"".to_string(),
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

        let mut context = MediaContext {
            r#type: "video".to_string(),
            time: defaultctx.time.clone(),
            source: defaultctx.source.clone(),
            space: defaultctx.space.clone(),
            meta: Default::default(),
        };

        context
            .meta
            .insert("ImageWidth".to_string(), Dynamic::from(1920i64));
        context
            .meta
            .insert("ImageHeight".to_string(), Dynamic::from(1080i64));
        context
            .meta
            .insert("duration".to_string(), Dynamic::from(120.5));

        let rule = Rule {
            condition: "type == \"image\"".to_string(),
            template: "{time.yyyy}/{time.mm}/{source.name}.{source.extension}".to_string(),
            action: ActionSpec::Copy,
        };

        let result = engine.process_rule(&rule, &context).unwrap();
        assert!(result.is_none());
    }
}
