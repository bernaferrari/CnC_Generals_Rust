// FILE: CleanupHazardUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, August 2002
// Desc:   Update module to handle independent targeting of hazards to cleanup.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CLEANUP_HAZARD_UPDATE_H_
#define __CLEANUP_HAZARD_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/KindOf.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ThingTemplate;
class WeaponTemplate;


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class CleanupHazardUpdateModuleData : public ModuleData
{
public:
	WeaponSlotType	m_weaponSlot;
	UnsignedInt			m_scanFrames;
	Real						m_scanRange;

	CleanupHazardUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class CleanupHazardUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CleanupHazardUpdate, "CleanupHazardUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CleanupHazardUpdate, CleanupHazardUpdateModuleData );

public:

	CleanupHazardUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();
	virtual UpdateSleepTime update();

	Object* scanClosestTarget();
	void fireWhenReady();

	void setCleanupAreaParameters( const Coord3D *pos, Real range ); //This allows the unit to cleanup an area until clean, then the AI goes idle.

protected:

	ObjectID m_bestTargetID;
	Bool m_inRange;
	Int m_nextScanFrames;
	Int m_nextShotAvailableInFrames;
	const WeaponTemplate *m_weaponTemplate;

	//Cleanup area (temporary values).
	Coord3D m_pos;
	Real		m_moveRange;
};


#endif

