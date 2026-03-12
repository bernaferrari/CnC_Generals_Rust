// FILE: AutoFindHealingUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: John Ahlquist, Sept. 2002
// Desc:   Update module to handle wounded idle infantry finding a heal unit or structure.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __AUTO_FIND_HEALING_UPDATE_H_
#define __AUTO_FIND_HEALING_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/KindOf.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ThingTemplate;
class WeaponTemplate;


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class AutoFindHealingUpdateModuleData : public ModuleData
{
public:
	UnsignedInt			m_scanFrames;
	Real						m_scanRange;
	Real						m_neverHeal;
	Real						m_alwaysHeal;

	AutoFindHealingUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class AutoFindHealingUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( AutoFindHealingUpdate, "AutoFindHealingUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( AutoFindHealingUpdate, AutoFindHealingUpdateModuleData );

public:

	AutoFindHealingUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();
	virtual UpdateSleepTime update();

	Object* scanClosestTarget();

protected:
	Int m_nextScanFrames;
};


#endif

