use std::{collections::HashMap, env::current_dir};

use chrono::Utc;
use duct::cmd;
use rrgen::RRgen;
use serde_json::json;

use crate::{errors::Error, Result};

const MODEL_T: &str = include_str!("templates/model.t");
const MODEL_TEST_T: &str = include_str!("templates/model_test.t");

use super::{collect_messages, AppInfo, MAPPINGS};

/// skipping some fields from the generated models.
/// For example, the `created_at` and `updated_at` fields are automatically
/// generated by the Loco app and should be given
pub const IGNORE_FIELDS: &[&str] = &["created_at", "updated_at", "create_at", "update_at"];

pub fn generate(
    rrgen: &RRgen,
    name: &str,
    is_link: bool,
    migration_only: bool,
    fields: &[(String, String)],
    appinfo: &AppInfo,
) -> Result<String> {
    let pkg_name: &str = &appinfo.app_name;
    let ts = Utc::now();

    let mut columns = Vec::new();
    let mut references = Vec::new();
    for (fname, ftype) in fields {
        if IGNORE_FIELDS.contains(&fname.as_str()) {
            tracing::warn!(
                field = fname,
                "note that a redundant field was specified, it is already generated automatically"
            );
            continue;
        }
        if ftype == "references" {
            let fkey = format!("{fname}_id");
            columns.push((fkey.clone(), "integer"));
            // user, user_id
            references.push((fname, fkey));
        } else {
            let schema_type = MAPPINGS.schema_field(ftype.as_str()).ok_or_else(|| {
                Error::Message(format!(
                    "type: {} not found. try any of: {:?}",
                    ftype,
                    MAPPINGS.schema_fields()
                ))
            })?;
            columns.push((fname.to_string(), schema_type.as_str()));
        }
    }

    let vars = json!({"name": name, "ts": ts, "pkg_name": pkg_name, "is_link": is_link, "columns": columns, "references": references});
    let res1 = rrgen.generate(MODEL_T, &vars)?;
    let res2 = rrgen.generate(MODEL_TEST_T, &vars)?;

    if !migration_only {
        let cwd = current_dir()?;
        let env_map: HashMap<_, _> = std::env::vars().collect();

        let _ = cmd!("cargo", "loco-tool", "db", "migrate",)
            .stderr_to_stdout()
            .dir(cwd.as_path())
            .full_env(&env_map)
            .run()
            .map_err(|err| {
                Error::Message(format!(
                    "failed to run loco db migration. error details: `{err}`",
                ))
            })?;
        let _ = cmd!("cargo", "loco-tool", "db", "entities",)
            .stderr_to_stdout()
            .dir(cwd.as_path())
            .full_env(&env_map)
            .run()
            .map_err(|err| {
                Error::Message(format!(
                    "failed to run loco db entities. error details: `{err}`",
                ))
            })?;
    }

    let messages = collect_messages(vec![res1, res2]);
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use std::env;

    #[test]
    fn test_can_generate_app() {
        let curdir = env::current_dir().unwrap();
        println!("current: {curdir:?}");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        println!("tmp: {path:?}");
        env::set_current_dir(path).unwrap();

        let newdir = env::current_dir().unwrap();
        println!("changed to: {newdir:?}");

        let cmd = "loco new -n saas -t saas --db sqlite --bg async --assets serverside";
        println!("RUN {cmd}");
        let result = duct_sh::sh_dangerous(cmd)
            .stderr_capture()
            .stderr_capture()
            .unchecked()
            .run()
            .unwrap();
        println!("result:\n{result:?}");

        env::set_current_dir(curdir).unwrap();
        panic!();
    }
}
