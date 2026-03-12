// FILE: DemoTrapUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, August 2002
// Desc:   Update module to handle demo trap proximity triggering.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DEMO_TRAP_UPDATE_H_
#define __DEMO_TRAP_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/KindOf.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class DemoTrapUpdateModuleData : public ModuleData
{
public:
	WeaponTemplate *m_detonationWeaponTemplate;
	KindOfMaskType	m_ignoreKindOf;
	WeaponSlotType  m_manualModeWeaponSlot;
	WeaponSlotType  m_detonationWeaponSlot;
	WeaponSlotType  m_proximityModeWeaponSlot;
	Real						m_triggerDetonationRange;
	UnsignedInt			m_scanFrames;
	Bool						m_defaultsToProximityMode;
	Bool						m_friendlyDetonation;
	Bool						m_detonateWhenKilled;
	
	DemoTrapUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class DemoTrapUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DemoTrapUpdate, "DemoTrapUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DemoTrapUpdate, DemoTrapUpdateModuleData );

public:

	DemoTrapUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();
	virtual UpdateSleepTime update();

	void detonate();

protected:

	Int m_nextScanFrames;
	Bool m_detonated;
};


#endif

