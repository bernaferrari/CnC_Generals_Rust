// FILE: PointDefenseLaserUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, August 2002
// Desc:   Update module to handle independent targeting of point defense laser.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __POINT_DEFENSE_LASER_UPDATE_H_
#define __POINT_DEFENSE_LASER_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/KindOf.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ThingTemplate;
class WeaponTemplate;


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class PointDefenseLaserUpdateModuleData : public ModuleData
{
public:
	WeaponTemplate	*m_weaponTemplate;
	KindOfMaskType	m_primaryTargetKindOf;
	KindOfMaskType  m_secondaryTargetKindOf;
	UnsignedInt			m_scanFrames;
	Real						m_scanRange;
	Real						m_velocityFactor;

	PointDefenseLaserUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class PointDefenseLaserUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PointDefenseLaserUpdate, "PointDefenseLaserUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( PointDefenseLaserUpdate, PointDefenseLaserUpdateModuleData );

public:

	PointDefenseLaserUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();
	virtual UpdateSleepTime update();

	Object* scanClosestTarget();
	void fireWhenReady();

protected:

	ObjectID m_bestTargetID;
	Bool m_inRange;
	Int m_nextScanFrames;
	Int m_nextShotAvailableInFrames;
};


#endif

