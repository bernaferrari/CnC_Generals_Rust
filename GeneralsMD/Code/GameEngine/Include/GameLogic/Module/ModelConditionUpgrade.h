// FILE: ModelConditionUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, July 2003
// Desc:	 UpgradeModule that sets a modelcondition flag
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _MODEL_CONDITION_UPGRADE_H
#define _MODEL_CONDITION_UPGRADE_H

#include "GameLogic/Module/UpgradeModule.h"

enum ModelConditionFlagType;
//-----------------------------------------------------------------------------
class ModelConditionUpgradeModuleData : public UpgradeModuleData
{
public:
	ModelConditionFlagType m_conditionFlag;

	ModelConditionUpgradeModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-----------------------------------------------------------------------------
class ModelConditionUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ModelConditionUpgrade, "ModelConditionUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ModelConditionUpgrade, ModelConditionUpgradeModuleData );

public:

	ModelConditionUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:
	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};
#endif // _MODEL_CONDITION_UPGRADE_H


