use std::{
    convert::Infallible,
    fmt::{self, Display},
    str::FromStr,
};

use eframe::{
    egui::{TextFormat, WidgetText},
    emath::Align,
    epaint::{text::LayoutJob, Color32},
};
use once_cell::sync::Lazy;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use string_interner::{symbol::DefaultSymbol, StringInterner};

static INTERNER: Lazy<RwLock<StringInterner>> = Lazy::new(|| RwLock::new(StringInterner::new()));

fn resolve(symbol: DefaultSymbol) -> MappedRwLockReadGuard<'static, str> {
    RwLockReadGuard::map(INTERNER.read(), |interner| {
        interner
            .resolve(symbol)
            .expect("string must have been interned")
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetricName {
    Namespace {
        namespace: DefaultSymbol,
        name: Box<MetricName>,
    },
    Name(DefaultSymbol),
}

#[macro_export]
macro_rules! metric_name {
    ($name:literal, $tt:tt) => {
        MetricName::namespace_static($name, metric_name!($tt))
    };
    ($name:literal) => {
        MetricName::name_static($name)
    };
}

impl MetricName {
    pub fn namespace_static(namespace: &'static str, name: MetricName) -> Self {
        Self::Namespace {
            namespace: INTERNER.write().get_or_intern_static(namespace),
            name: Box::new(name),
        }
    }

    pub fn namespace(namespace: &str, name: MetricName) -> Self {
        Self::Namespace {
            namespace: INTERNER.write().get_or_intern(namespace),
            name: Box::new(name),
        }
    }

    pub fn name_static(name: &'static str) -> Self {
        Self::Name(INTERNER.write().get_or_intern_static(name))
    }

    pub fn name(name: &str) -> Self {
        Self::Name(INTERNER.write().get_or_intern(name))
    }

    pub fn flatten(&self) -> Flatten {
        Flatten { name: Some(self) }
    }
}

pub struct Flatten<'n> {
    name: Option<&'n MetricName>,
}

impl<'n> Iterator for Flatten<'n> {
    type Item = MappedRwLockReadGuard<'n, str>;

    fn next(&mut self) -> Option<Self::Item> {
        let symbol = match self.name? {
            MetricName::Namespace { namespace, name } => {
                self.name = Some(name);

                *namespace
            }
            MetricName::Name(name) => {
                self.name = None;

                *name
            }
        };

        Some(resolve(symbol))
    }
}

impl Display for MetricName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = self
            .flatten()
            .fold(String::new(), |pre, new| pre + ":" + new.as_ref());

        write!(f, "{string}")
    }
}

impl FromStr for MetricName {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split(':');

        // Get the metric name
        let name = split
            .next_back()
            .expect("split iterator must have at least one element");

        // Fold the namespaces from the back
        Ok(split.rfold(MetricName::name(name), |name, namespace| {
            MetricName::namespace(namespace, name)
        }))
    }
}

impl From<&MetricName> for WidgetText {
    fn from(metric_name: &MetricName) -> Self {
        let mut job = LayoutJob::default();

        let mut metric_name = metric_name;

        loop {
            match metric_name {
                MetricName::Namespace { namespace, name } => {
                    job.append(
                        resolve(*namespace).as_ref(),
                        0.0,
                        TextFormat {
                            color: Color32::KHAKI,
                            valign: Align::Center,
                            ..Default::default()
                        },
                    );
                    job.append(
                        ":",
                        0.0,
                        TextFormat {
                            valign: Align::Center,
                            ..Default::default()
                        },
                    );

                    metric_name = name;
                }
                MetricName::Name(name) => {
                    job.append(
                        resolve(*name).as_ref(),
                        0.0,
                        TextFormat {
                            color: Color32::GOLD,
                            valign: Align::Center,
                            ..Default::default()
                        },
                    );

                    // laid out the whole metric name, return
                    return WidgetText::LayoutJob(job);
                }
            }
        }
    }
}
