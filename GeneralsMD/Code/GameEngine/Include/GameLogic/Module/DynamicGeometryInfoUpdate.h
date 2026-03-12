// FILE: DynamicGeometryInfoUpdate.h //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, April 2002
// Desc:   Update module that changes the object's GeometryInfo
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DYNAMIC_GEOMETRY_INFO_UPDATE_H_
#define __DYNAMIC_GEOMETRY_INFO_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Geometry.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class DynamicGeometryInfoUpdateModuleData : public ModuleData
{
public:

	UnsignedInt m_initialDelay;

	Real m_initialHeight;
	Real m_initialMajorRadius;
	Real m_initialMinorRadius;

	Real m_finalHeight;
	Real m_finalMajorRadius;
	Real m_finalMinorRadius;

	UnsignedInt m_transitionTime;

	Bool m_reverseAtTransitionTime;		///< reverse directions once transition time is reached

	// I will go from initial to final in transitionTime frames, smoothly.
	// I won't change type until that is actually needed as a task.

	DynamicGeometryInfoUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class DynamicGeometryInfoUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DynamicGeometryInfoUpdate, "DynamicGeometryInfoUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DynamicGeometryInfoUpdate, DynamicGeometryInfoUpdateModuleData );

public:

	DynamicGeometryInfoUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

protected:

	UnsignedInt m_startingDelayCountdown;
	UnsignedInt m_timeActive;

	Bool m_started;
	Bool m_finished;

	Bool m_reverseAtTransitionTime;						///< do a reverse at transition time
	enum DynamicGeometryDirection { FORWARD = 1, BACKWARD = -1 };
	DynamicGeometryDirection m_direction;			///< direction we're growing/shrinking
	Bool m_switchedDirections;								///< TRUE once we've switched directions

	Real m_initialHeight;
	Real m_initialMajorRadius;
	Real m_initialMinorRadius;
	Real m_finalHeight;
	Real m_finalMajorRadius;
	Real m_finalMinorRadius;

};


#endif

