// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cost module tests

use guestkit::cli::cost::*;

#[cfg(test)]
mod cloud_provider_tests {
    use super::*;

    #[test]
    fn test_cloud_provider_from_str() {
        assert_eq!(CloudProvider::from_str("aws"), Some(CloudProvider::AWS));
        assert_eq!(CloudProvider::from_str("amazon"), Some(CloudProvider::AWS));
        assert_eq!(CloudProvider::from_str("azure"), Some(CloudProvider::Azure));
        assert_eq!(CloudProvider::from_str("microsoft"), Some(CloudProvider::Azure));
        assert_eq!(CloudProvider::from_str("gcp"), Some(CloudProvider::GCP));
        assert_eq!(CloudProvider::from_str("google"), Some(CloudProvider::GCP));
        assert_eq!(CloudProvider::from_str("invalid"), None);
    }

    #[test]
    fn test_cloud_provider_case_insensitive() {
        assert_eq!(CloudProvider::from_str("AWS"), Some(CloudProvider::AWS));
        assert_eq!(CloudProvider::from_str("Azure"), Some(CloudProvider::Azure));
        assert_eq!(CloudProvider::from_str("GCP"), Some(CloudProvider::GCP));
    }
}

#[cfg(test)]
mod workload_profile_tests {
    use super::*;

    #[test]
    fn test_workload_profile_from_str() {
        assert_eq!(WorkloadProfile::from_str("web"), Some(WorkloadProfile::WebServer));
        assert_eq!(WorkloadProfile::from_str("database"), Some(WorkloadProfile::Database));
        assert_eq!(WorkloadProfile::from_str("compute"), Some(WorkloadProfile::ComputeIntensive));
        assert_eq!(WorkloadProfile::from_str("memory"), Some(WorkloadProfile::MemoryIntensive));
        assert_eq!(WorkloadProfile::from_str("storage"), Some(WorkloadProfile::StorageIntensive));
        assert_eq!(WorkloadProfile::from_str("batch"), Some(WorkloadProfile::Batch));
        assert_eq!(WorkloadProfile::from_str("general"), Some(WorkloadProfile::General));
        assert_eq!(WorkloadProfile::from_str("invalid"), None);
    }
}

#[cfg(test)]
mod estimator_tests {
    use super::*;

    fn create_test_metrics() -> SystemMetrics {
        SystemMetrics {
            cpu_cores: 4,
            memory_gb: 16.0,
            storage_gb: 100.0,
            network_gb_month: 500.0,
        }
    }

    fn create_test_profile() -> WorkloadProfile {
        WorkloadProfile::WebServer
    }

    #[test]
    fn test_estimate_current_costs_aws() {
        let metrics = create_test_metrics();
        let profile = create_test_profile();
        let estimate = estimator::estimate_current_costs(
            &metrics,
            CloudProvider::AWS,
            "us-east-1",
            &profile,
        );

        assert!(estimate.compute_monthly > 0.0);
        assert!(estimate.storage_monthly > 0.0);
        assert!(estimate.network_monthly > 0.0);
        assert_eq!(
            estimate.total_monthly,
            estimate.compute_monthly + estimate.storage_monthly + estimate.network_monthly
        );
        assert_eq!(estimate.total_annual, estimate.total_monthly * 12.0);
    }

    #[test]
    fn test_estimate_current_costs_azure() {
        let metrics = create_test_metrics();
        let profile = create_test_profile();
        let estimate = estimator::estimate_current_costs(
            &metrics,
            CloudProvider::Azure,
            "eastus",
            &profile,
        );

        assert!(estimate.compute_monthly > 0.0);
        assert!(estimate.total_monthly > 0.0);
    }

    #[test]
    fn test_estimate_current_costs_gcp() {
        let metrics = create_test_metrics();
        let profile = create_test_profile();
        let estimate = estimator::estimate_current_costs(
            &metrics,
            CloudProvider::GCP,
            "us-central1",
            &profile,
        );

        assert!(estimate.compute_monthly > 0.0);
        assert!(estimate.total_monthly > 0.0);
    }

    #[test]
    fn test_estimate_optimized_costs() {
        let metrics = create_test_metrics();
        let profile = create_test_profile();
        let estimate = estimator::estimate_optimized_costs(
            &metrics,
            CloudProvider::AWS,
            "us-east-1",
            &profile,
        );

        // Optimized should be cheaper
        assert!(estimate.compute_monthly >= 0.0);
        assert!(estimate.total_monthly > 0.0);
    }
}

#[cfg(test)]
mod analyzer_tests {
    use super::*;

    fn create_test_analysis() -> CostAnalysis {
        CostAnalysis {
            provider: CloudProvider::AWS,
            region: "us-east-1".to_string(),
            workload_profile: WorkloadProfile::WebServer,
            current_estimate: ResourceEstimate {
                compute_monthly: 200.0,
                storage_monthly: 50.0,
                network_monthly: 30.0,
                total_monthly: 280.0,
                total_annual: 3360.0,
                instance_type: "t3.xlarge".to_string(),
                instance_count: 2,
            },
            optimized_estimate: ResourceEstimate {
                compute_monthly: 120.0,
                storage_monthly: 30.0,
                network_monthly: 25.0,
                total_monthly: 175.0,
                total_annual: 2100.0,
                instance_type: "t3.large".to_string(),
                instance_count: 2,
            },
            savings_opportunities: vec![],
            recommendations: vec![],
            total_savings_monthly: 105.0,
            total_savings_annual: 1260.0,
            savings_percentage: 37.5,
        }
    }

    #[test]
    fn test_find_savings_opportunities() {
        let analysis = create_test_analysis();
        let opportunities = analyzer::find_savings_opportunities(&analysis);

        assert!(!opportunities.is_empty());
        // Should find at least right-sizing opportunity
        assert!(opportunities.iter().any(|o| o.category == "Right-sizing"));
    }

    #[test]
    fn test_savings_opportunity_priority() {
        let analysis = create_test_analysis();
        let opportunities = analyzer::find_savings_opportunities(&analysis);

        // High savings should have High priority
        for opp in opportunities.iter() {
            if opp.estimated_savings_monthly > 100.0 {
                assert_eq!(opp.priority, Priority::High);
            }
        }
    }

    #[test]
    fn test_generate_recommendations_aws() {
        let analysis = create_test_analysis();
        let recommendations = analyzer::generate_recommendations(&analysis);

        assert!(!recommendations.is_empty());
        // AWS should include Graviton recommendation
        assert!(recommendations
            .iter()
            .any(|r| r.contains("Graviton") || r.contains("Reserved Instances")));
    }

    #[test]
    fn test_generate_recommendations_azure() {
        let mut analysis = create_test_analysis();
        analysis.provider = CloudProvider::Azure;
        let recommendations = analyzer::generate_recommendations(&analysis);

        assert!(!recommendations.is_empty());
        // Azure should include hybrid benefit
        assert!(recommendations
            .iter()
            .any(|r| r.contains("Hybrid Benefit") || r.contains("Reserved")));
    }

    #[test]
    fn test_generate_recommendations_gcp() {
        let mut analysis = create_test_analysis();
        analysis.provider = CloudProvider::GCP;
        let recommendations = analyzer::generate_recommendations(&analysis);

        assert!(!recommendations.is_empty());
        // GCP should include CUD or preemptible
        assert!(recommendations.iter().any(
            |r| r.contains("Committed Use") || r.contains("Preemptible") || r.contains("Spot")
        ));
    }
}

#[cfg(test)]
mod reporter_tests {
    use super::*;

    fn create_test_analysis() -> CostAnalysis {
        CostAnalysis {
            provider: CloudProvider::AWS,
            region: "us-east-1".to_string(),
            workload_profile: WorkloadProfile::WebServer,
            current_estimate: ResourceEstimate {
                compute_monthly: 200.0,
                storage_monthly: 50.0,
                network_monthly: 30.0,
                total_monthly: 280.0,
                total_annual: 3360.0,
                instance_type: "t3.xlarge".to_string(),
                instance_count: 2,
            },
            optimized_estimate: ResourceEstimate {
                compute_monthly: 120.0,
                storage_monthly: 30.0,
                network_monthly: 25.0,
                total_monthly: 175.0,
                total_annual: 2100.0,
                instance_type: "t3.large".to_string(),
                instance_count: 2,
            },
            savings_opportunities: vec![SavingsOpportunity {
                category: "Right-sizing".to_string(),
                description: "Reduce instance size".to_string(),
                estimated_savings_monthly: 80.0,
                estimated_savings_annual: 960.0,
                implementation_effort: "Low".to_string(),
                priority: Priority::High,
                steps: vec!["Resize instances".to_string()],
            }],
            recommendations: vec!["Use Reserved Instances".to_string()],
            total_savings_monthly: 105.0,
            total_savings_annual: 1260.0,
            savings_percentage: 37.5,
        }
    }

    #[test]
    fn test_format_report_contains_header() {
        let analysis = create_test_analysis();
        let report = reporter::format_report(&analysis, false);

        assert!(report.contains("Cloud Cost Analysis Report"));
        assert!(report.contains("AWS"));
        assert!(report.contains("us-east-1"));
    }

    #[test]
    fn test_format_report_contains_estimates() {
        let analysis = create_test_analysis();
        let report = reporter::format_report(&analysis, false);

        assert!(report.contains("Current Costs"));
        assert!(report.contains("Optimized Costs"));
        assert!(report.contains("$280.00"));
        assert!(report.contains("$175.00"));
    }

    #[test]
    fn test_format_report_contains_savings() {
        let analysis = create_test_analysis();
        let report = reporter::format_report(&analysis, false);

        assert!(report.contains("Savings Opportunities"));
        assert!(report.contains("Right-sizing"));
        assert!(report.contains("37.5%"));
    }

    #[test]
    fn test_format_report_contains_recommendations() {
        let analysis = create_test_analysis();
        let report = reporter::format_report(&analysis, false);

        assert!(report.contains("Recommendations"));
        assert!(report.contains("Use Reserved Instances"));
    }

    #[test]
    fn test_format_csv_header() {
        let analysis = create_test_analysis();
        let csv = reporter::format_csv(&analysis);

        assert!(csv.starts_with("Category,Description"));
    }

    #[test]
    fn test_format_csv_content() {
        let analysis = create_test_analysis();
        let csv = reporter::format_csv(&analysis);

        assert!(csv.contains("Right-sizing"));
        assert!(csv.contains("80.00"));
        assert!(csv.contains("960.00"));
    }
}

#[cfg(test)]
mod priority_tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
        assert_eq!(Priority::High, Priority::High);
    }
}
