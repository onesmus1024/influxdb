use arrow::datatypes::DataType;
use arrow_util::assert_batches_eq;
use datafusion::logical_plan::{col, lit};
use predicate::predicate::{Predicate, PredicateBuilder};
use query::{
    exec::{
        fieldlist::{Field, FieldList},
        ExecutorType,
    },
    frontend::influxrpc::InfluxRpcPlanner,
};

use crate::scenarios::*;

/// Creates and loads several database scenarios using the db_setup
/// function.
///
/// runs field_column_names(predicate) and compares it to the expected
/// output
async fn run_field_columns_test_case<D>(
    db_setup: D,
    predicate: Predicate,
    expected_fields: FieldList,
) where
    D: DbSetup,
{
    test_helpers::maybe_start_logging();

    for scenario in db_setup.make().await {
        let DbScenario {
            scenario_name, db, ..
        } = scenario;
        println!("Running scenario '{}'", scenario_name);
        println!("Predicate: '{:#?}'", predicate);
        let planner = InfluxRpcPlanner::new();
        let ctx = db.executor().new_context(query::exec::ExecutorType::Query);

        let plan = planner
            .field_columns(db.as_ref(), predicate.clone())
            .expect("built plan successfully");
        let fields = ctx
            .to_field_list(plan)
            .await
            .expect("converted plan to strings successfully");

        assert_eq!(
            fields, expected_fields,
            "Error in  scenario '{}'\n\nexpected:\n{:#?}\nactual:\n{:#?}",
            scenario_name, expected_fields, fields
        );
    }
}

#[tokio::test]
async fn test_field_columns_empty_database() {
    let predicate = PredicateBuilder::default().build();
    let expected_fields = FieldList::default();
    run_field_columns_test_case(NoData {}, predicate, expected_fields).await;
}

#[tokio::test]
async fn test_field_columns_no_predicate() {
    let predicate = PredicateBuilder::default()
        .table("NoSuchTable")
        .add_expr(col("state").eq(lit("MA"))) // state=MA
        .build();
    let expected_fields = FieldList::default();
    run_field_columns_test_case(TwoMeasurementsManyFields {}, predicate, expected_fields).await;
}

// NGA todo: add delete tests when the TwoMeasurementsManyFieldsWithDelete available

#[tokio::test]
async fn test_field_columns_with_pred() {
    // get only fields from h20 (but both chunks)
    let predicate = PredicateBuilder::default()
        .table("h2o")
        .add_expr(col("state").eq(lit("MA"))) // state=MA
        .build();

    let expected_fields = FieldList {
        fields: vec![
            Field {
                name: "moisture".into(),
                data_type: DataType::Float64,
                last_timestamp: 100000,
            },
            Field {
                name: "other_temp".into(),
                data_type: DataType::Float64,
                last_timestamp: 250,
            },
            Field {
                name: "temp".into(),
                data_type: DataType::Float64,
                last_timestamp: 100000,
            },
        ],
    };

    run_field_columns_test_case(TwoMeasurementsManyFields {}, predicate, expected_fields).await;
}

#[tokio::test]
async fn test_field_columns_measurement_pred() {
    // get only fields from h2o using a _measurement predicate
    let predicate = PredicateBuilder::default()
        .add_expr(col("_measurement").eq(lit("h2o")))
        .build();

    let expected_fields = FieldList {
        fields: vec![
            Field {
                name: "moisture".into(),
                data_type: DataType::Float64,
                last_timestamp: 100000,
            },
            Field {
                name: "other_temp".into(),
                data_type: DataType::Float64,
                last_timestamp: 350,
            },
            Field {
                name: "temp".into(),
                data_type: DataType::Float64,
                last_timestamp: 100000,
            },
        ],
    };

    run_field_columns_test_case(TwoMeasurementsManyFields {}, predicate, expected_fields).await;
}

#[tokio::test]
async fn test_field_columns_with_ts_pred() {
    let predicate = PredicateBuilder::default()
        .table("h2o")
        .timestamp_range(200, 300)
        .add_expr(col("state").eq(lit("MA"))) // state=MA
        .build();

    let expected_fields = FieldList {
        fields: vec![Field {
            name: "other_temp".into(),
            data_type: DataType::Float64,
            last_timestamp: 250,
        }],
    };

    run_field_columns_test_case(TwoMeasurementsManyFields {}, predicate, expected_fields).await;
}

#[tokio::test]
async fn test_field_name_plan() {
    test_helpers::maybe_start_logging();
    // Tests that the ordering that comes out is reasonable
    let scenarios = OneMeasurementManyFields {}.make().await;

    for scenario in scenarios {
        let predicate = PredicateBuilder::default().timestamp_range(0, 200).build();

        let DbScenario {
            scenario_name, db, ..
        } = scenario;
        println!("Running scenario '{}'", scenario_name);
        println!("Predicate: '{:#?}'", predicate);
        let planner = InfluxRpcPlanner::new();
        let ctx = db.executor().new_context(ExecutorType::Query);

        let plan = planner
            .field_columns(db.as_ref(), predicate.clone())
            .expect("built plan successfully");

        let mut plans = plan.plans;
        let plan = plans.pop().unwrap();
        assert!(plans.is_empty()); // only one plan

        // run the created plan directly, ensuring the output is as
        // expected (specifically that the column ordering is correct)
        let results = ctx.run_logical_plan(plan).await.expect("ok running plan");

        let expected = vec![
            "+--------+--------+--------+--------+--------------------------------+",
            "| field1 | field2 | field3 | field4 | time                           |",
            "+--------+--------+--------+--------+--------------------------------+",
            "| 70.5   | ss     | 2      |        | 1970-01-01T00:00:00.000000100Z |",
            "+--------+--------+--------+--------+--------------------------------+",
        ];

        assert_batches_eq!(expected, &results);
    }
}

// BUG: https://github.com/influxdata/influxdb_iox/issues/2860
#[ignore]
#[tokio::test]
async fn test_field_name_plan_with_delete() {
    test_helpers::maybe_start_logging();
    // Tests that the ordering that comes out is reasonable
    let scenarios = OneMeasurementManyFieldsWithDelete {}.make().await;

    for scenario in scenarios {
        let predicate = PredicateBuilder::default().timestamp_range(0, 200).build();

        let DbScenario {
            scenario_name, db, ..
        } = scenario;
        println!("Running scenario '{}'", scenario_name);
        println!("Predicate: '{:#?}'", predicate);
        let planner = InfluxRpcPlanner::new();
        let ctx = db.executor().new_context(ExecutorType::Query);

        let plan = planner
            .field_columns(db.as_ref(), predicate.clone())
            .expect("built plan successfully");

        let mut plans = plan.plans;
        let plan = plans.pop().unwrap();
        assert!(plans.is_empty()); // only one plan

        // run the created plan directly, ensuring the output is as
        // expected (specifically that the column ordering is correct)
        let results = ctx.run_logical_plan(plan).await.expect("ok running plan");

        // Todo: After the panic bug is fixed, this result should be recheck. I think column field4 will disappear from the result
        let expected = vec![
            "+--------+--------+--------+--------+--------------------------------+",
            "| field1 | field2 | field3 | field4 | time                           |",
            "+--------+--------+--------+--------+--------------------------------+",
            "| 70.5   | ss     | 2      |        | 1970-01-01T00:00:00.000000100Z |",
            "+--------+--------+--------+--------+--------------------------------+",
        ];

        assert_batches_eq!(expected, &results);
    }
}
