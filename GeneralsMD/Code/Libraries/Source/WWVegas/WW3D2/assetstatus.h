#if defined(_MSC_VER)
#pragma once
#endif

#ifndef WW3D_ASSET_STATUS_H
#define WW3D_ASSET_STATUS_H

#include "always.h"
#include "hashtemplate.h"

class AssetStatusClass
{
public:
	enum {
		REPORT_LOAD_ON_DEMAND_ROBJ,
		REPORT_LOAD_ON_DEMAND_HANIM,
		REPORT_LOAD_ON_DEMAND_HTREE,
		REPORT_MISSING_ROBJ,
		REPORT_MISSING_HANIM,
		REPORT_MISSING_HTREE,
		REPORT_COUNT
	};

	AssetStatusClass();
	~AssetStatusClass();

	void Enable_Reporting(bool enable)					  { Reporting=enable; }	
	void Enable_Load_On_Demand_Reporting(bool enable) { LoadOnDemandReporting=enable; }

	void Report_Load_On_Demand_RObj(const char* name);
	void Report_Load_On_Demand_HAnim(const char* name);
	void Report_Load_On_Demand_HTree(const char* name);

	void Report_Missing_RObj(const char* name);
	void Report_Missing_HAnim(const char* name);
	void Report_Missing_HTree(const char* name);

	static AssetStatusClass* Peek_Instance() { return &Instance; }

private:
	bool Reporting;
	bool LoadOnDemandReporting;
	static AssetStatusClass Instance;
	HashTemplateClass<StringClass, int> ReportHashTables[REPORT_COUNT];

	void Add_To_Report(int index, const char* name);

};

#endif
