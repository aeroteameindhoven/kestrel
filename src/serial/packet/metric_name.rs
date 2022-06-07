use std::{
    convert::Infallible,
    fmt::{self, Display},
    str::FromStr,
};

use eframe::{
    egui::{RichText, TextFormat, WidgetText},
    epaint::{text::LayoutJob, Color32},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetricName {
    Namespaced { namespace: String, name: String },
    Global(String),
}

impl MetricName {
    pub fn namespaced(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Namespaced {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    pub fn global(name: impl Into<String>) -> Self {
        Self::Global(name.into())
    }
}

impl Display for MetricName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricName::Namespaced { namespace, name } => write!(f, "{namespace}:{name}"),
            MetricName::Global(name) => Display::fmt(name, f),
        }
    }
}

impl FromStr for MetricName {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.split_once(':') {
            Some((namespace, name)) => Self::Namespaced {
                namespace: namespace.into(),
                name: name.into(),
            },
            None => Self::Global(s.into()),
        })
    }
}

impl From<&MetricName> for WidgetText {
    fn from(metric_name: &MetricName) -> Self {
        match metric_name {
            MetricName::Namespaced { namespace, name } => {
                let mut job = LayoutJob::default();
                job.append(
                    namespace,
                    0.0,
                    TextFormat {
                        color: Color32::KHAKI,
                        ..Default::default()
                    },
                );
                job.append(":", 0.0, Default::default());
                job.append(
                    name,
                    0.0,
                    TextFormat {
                        color: Color32::GOLD,
                        ..Default::default()
                    },
                );
                WidgetText::LayoutJob(job)
            }
            MetricName::Global(name) => {
                WidgetText::RichText(RichText::new(name).color(Color32::GOLD))
            }
        }
    }
}
