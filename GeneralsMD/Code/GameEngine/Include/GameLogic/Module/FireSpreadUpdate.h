// FILE: FireSpreadUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, April 2002
// Desc:   Update looks for ::Aflame and explicitly ignites someone nearby if set
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FIRE_SPREAD_UPDATE_H_
#define __FIRE_SPREAD_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

class ObjectCreationList;

//-------------------------------------------------------------------------------------------------
class FireSpreadUpdateModuleData : public UpdateModuleData
{
public:
	const ObjectCreationList *m_oclEmbers;
	UnsignedInt m_minSpreadTryDelayData;
	UnsignedInt m_maxSpreadTryDelayData;
	Real m_spreadTryRange;

	FireSpreadUpdateModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);

private:

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class FireSpreadUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FireSpreadUpdate, "FireSpreadUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FireSpreadUpdate, FireSpreadUpdateModuleData )

public:

	FireSpreadUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

	void startFireSpreading();

protected:
	
	UnsignedInt calcNextSpreadDelay();

};

#endif

