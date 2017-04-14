use std::*;
use rusqlite as sqlite;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, Value, ValueRef};
use regex::Regex;


/// When a track or album has been released. The items of this enum indicate the level of
/// precision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum Release {
    Year{ year: u32 },
    Month{ year: u32, month: u32 },
    Day{ year: u32, month: u32, day: u32 },
}

impl Release {
    /// Takes the most precise of this and another Release in `Year < Month < Day`.
    /// There is no requirement for `self == other`.
    /// If the releases have the same level of accuracy, self is returned.
    pub fn most_precise(self, other: Release) -> Release {
        match (&self, &other) {
            (_, &Release::Year{ .. }) => self,
            (&Release::Year{ .. }, _) => other,
            (&Release::Day{ .. }, _) => self,
            (_, &Release::Day{ .. }) => other,
            _ => self,
        }
    }

    pub fn year(&self) -> u32 {
        match *self {
            Release::Year{ year } => year,
            Release::Month{ year, .. } => year,
            Release::Day{ year, .. } => year,
        }
    }

    pub fn month(&self) -> Option<u32> {
        match *self {
            Release::Year{ .. } => None,
            Release::Month{ month, .. } => Some(month),
            Release::Day{ month, .. } => Some(month),
        }
    }

    pub fn day(&self) -> Option<u32> {
        match *self {
            Release::Year{ .. } => None,
            Release::Month{ .. } => None,
            Release::Day{ day, .. } => Some(day),
        }
    }
}

impl Ord for Release {
    fn cmp(&self, other: &Release) -> cmp::Ordering {
        let y = self.year().cmp(&other.year());
        if y != cmp::Ordering::Equal {
            return y;
        }
        let m = self.month().unwrap_or(0)
            .cmp(&other.month().unwrap_or(0));
        if m != cmp::Ordering::Equal {
            return m;
        }
        self.day().unwrap_or(0)
            .cmp(&other.day().unwrap_or(0))
    }
}

impl str::FromStr for Release {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Release, ParseError> {
        lazy_static! {
            static ref DATE_LE: Regex = Regex::new(r"(?x)
                (^|\s)
                (?:
                    (?:
                        (?P<d>[1-3]\d) [-\./]
                    )?
                    (?P<m>[12]?[1-9]\d) [-\./]
                )?
                (?P<y>\d{4})
            ").unwrap();
            static ref DATE_BE: Regex = Regex::new(r"(?x)
                (^|\s)
                (?P<y>\d{4})
                (?:
                    [-\./] (?P<m>[12]?[1-9]\d)
                    (?:
                        [-\./] (?P<d>[1-3]\d)
                    )?
                )?
            ").unwrap();
        }

        DATE_BE.captures(s)
            .or_else(|| DATE_LE.captures(s))
            .and_then(|mat| {
                let y = mat.name("y")
                    .and_then(|m| m.as_str().parse().ok());
                let m = mat.name("m")
                    .and_then(|m| m.as_str().parse().ok())
                    .and_then(|m| match m {
                        1 ... 12 => Some(m),
                        _ => None,
                    });
                let d = mat.name("d")
                    .and_then(|d| d.as_str().parse().ok())
                    .and_then(|d| match d {
                        1 ... 31 => Some(d),
                        _ => None,
                    });

                match (y, m, d) {
                    (Some(y), Some(m), Some(d)) => Some(Release::Day{
                        year: y,
                        month: m,
                        day: d,
                    }),
                    (Some(y), Some(m), _) => Some(Release::Month{
                        year: y,
                        month: m,
                    }),
                    (Some(y), _, _) => Some(Release::Year{
                        year: y,
                    }),
                    _ => None,
                }
            })
            .map(|r| Ok(r))
            .unwrap_or(Err(ParseError::Unmatched))
    }
}

impl ToSql for Release {
    fn to_sql(&self) -> Result<ToSqlOutput, sqlite::Error> {
        let s = match *self {
            Release::Year{ year } => format!("{:04}", year),
            Release::Month{ year, month } => format!("{:04}-{:02}", year, month),
            Release::Day{ year, month, day } => format!("{:04}-{:02}-{:02}", year, month, day),
        };
        Ok(ToSqlOutput::Owned(Value::Text(s)))
    }
}

impl FromSql for Release {
    fn column_result(value: ValueRef) -> FromSqlResult<Release> {
        value.as_str()?
            .parse()
            .map_err(|err| FromSqlError::Other(Box::from(err)))
    }
}


#[derive(Debug)]
pub enum ParseError {
    Unmatched,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::Unmatched =>  {
                write!(f, "Input text is was not matched")
            },
        }
    }
}

impl error::Error for ParseError {
    fn description(&self) -> &str {
        "Release parse error"
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year() {
        assert_eq!(Release::Year{year: 2012}, "  2012".parse().unwrap());
        assert_eq!(Release::Year{year: 2012}, "2012 ".parse().unwrap());
        assert_eq!(Release::Year{year: 2012}, " 2012 ".parse().unwrap());
        assert_eq!(Release::Year{year: 0}, "0000".parse().unwrap());
        assert_eq!(Release::Year{year: 12}, "0012".parse().unwrap());
        assert_eq!(Release::Year{year: 1984}, "1984".parse().unwrap());
        assert_eq!(Release::Year{year: 2017}, "2017".parse().unwrap());
        assert_eq!(Release::Year{year: 9999}, "9999".parse().unwrap());
        assert_eq!(Release::Year{year: 2017}, "2017-00-00".parse().unwrap());
    }

    #[test]
    fn year_bad() {
        assert!("-0000".parse::<Release>().is_err());
        assert!("0".parse::<Release>().is_err());
        assert!("-0".parse::<Release>().is_err());
        assert!("1".parse::<Release>().is_err());
        assert!("24".parse::<Release>().is_err());
        assert!("666".parse::<Release>().is_err());
        assert!("foo".parse::<Release>().is_err());
        assert!("".parse::<Release>().is_err());
        assert!("yyyy".parse::<Release>().is_err());
        assert!(" ".parse::<Release>().is_err());
    }

    // TODO
}
