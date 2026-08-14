use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", content = "props")]
pub enum GenerativeWidget {
    MetricCard {
        label: String,
        value: String,
        unit: Option<String>,
        status: Option<String>,
    },
    SensorGauge {
        label: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        unit: Option<String>,
    },
    StatusList {
        title: String,
        items: Vec<StatusItem>,
    },
    Chart {
        title: String,
        data: Vec<ChartDataPoint>,
        chart_type: String,
    },
    ActionForm {
        action_name: String,
        description: String,
        fields: Vec<FormField>,
        risk_level: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StatusItem {
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub placeholder: Option<String>,
    pub required: bool,
}
