use crate::compiler::prelude::*;
use chrono::DateTime;
use gostd_time::Location;

fn parse_go_timestamp(
    value: Value,
    formats: &Vec<String>,
    timezone: &Location
) -> Resolved {
    match value {
        Value::Bytes(v) => {
            let value = String::from_utf8_lossy(v.as_ref());
            for format in formats {
                if let Ok(t) = gostd_time::ParseInLocation(format, &value, &timezone) {
                    return Ok(Value::Timestamp(
                        DateTime::from_timestamp(t.Unix(), t.Nanosecond() as u32).unwrap(),
                    ));
                }
            }
            Err("unable to convert value to timestamp".into())
        }
        Value::Timestamp(_) => Ok(value),
        _ => Err("unable to convert value to timestamp".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseGoTimestamp;

impl Function for ParseGoTimestamp {
    fn identifier(&self) -> &'static str {
        "parse_go_timestamp"
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "valid",
                source: r#"parse_go_timestamp!("11-Feb-2021 16:00 +00:00", format: "02-Jan-2006 15:04 +07:00")"#,
                result: Ok("t'2021-02-11T16:00:00Z'"),
            },
            Example {
                title: "valid with timezone",
                source: r#"parse_go_timestamp!("16/10/2019 12:00:00", format: "02/01/2006 15:04:05", timezone: "Europe/Paris")"#,
                result: Ok("t'2019-10-16T10:00:00Z'"),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let formats = arguments
            .required_array("formats")?
            .into_iter()
            .map(|expr| {
                let pattern = expr
                    .resolve_constant(state)
                    .ok_or(function::Error::ExpectedStaticExpression {
                        keyword: "formats",
                        expr: expr.clone(),
                    })?
                    .try_bytes_utf8_lossy()
                    .map_err(|_| function::Error::InvalidArgument {
                        keyword: "formats",
                        value: format!("{expr:?}").into(),
                        error: "go_timestamp formats should be a string array",
                    })?
                    .into_owned();
                Ok(pattern)
            })
            .collect::<std::result::Result<Vec<String>, function::Error>>()?;

        let timezone_expr = arguments.required_expr("timezone");
        let tz = timezone_expr
            .resolve_constant(state)
            .ok_or(function::Error::ExpectedStaticExpression {
                keyword: "timezone",
                expr: timezone_expr.clone(),
            })?
            .try_bytes_utf8_lossy()
            .map_err(|_| function::Error::InvalidArgument {
                keyword: "timezone",
                value: format!("{timezone_expr:?}").into(),
                error: "go_timestamp timezone should be a string",
            })?
            .into_owned();
        let loc = gostd_time::LoadLocation(&tz).map_err(|_| function::Error::InvalidArgument {
            keyword: "timezone",
            value: format!("{timezone_expr:?}").into(),
            error: "go_timestamp timezone should be a legal timezone",
        })?;

        Ok(ParseGoTimestampFn {
            value,
            formats,
            loc,
        }
        .as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES | kind::TIMESTAMP,
                required: true,
            },
            Parameter {
                keyword: "formats",
                kind: kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "timezone",
                kind: kind::BYTES,
                required: true,
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct ParseGoTimestampFn {
    value: Box<dyn Expression>,
    formats: Vec<String>,
    loc: Location,
}

impl FunctionExpression for ParseGoTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_go_timestamp(value, &self.formats, &self.loc)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::timestamp().fallible(/* always fallible because the format and the timezone need to be parsed at runtime */)
    }
}
