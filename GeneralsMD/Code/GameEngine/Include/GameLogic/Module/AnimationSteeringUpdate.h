// FILE: AnimationSteeringUpdate.h ////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, April 2002
// Desc:   Uses animation states to handle steering.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __ANIMATION_STEERING_UPDATE_H
#define __ANIMATION_STEERING_UPDATE_H

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

enum PhysicsTurningType;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class AnimationSteeringUpdateModuleData : public UpdateModuleData
{

public:

	AnimationSteeringUpdateModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse( p );

		static const FieldParse dataFieldParse[] = 
		{
			{ "MinTransitionTime", INI::parseDurationUnsignedInt, NULL, offsetof( AnimationSteeringUpdateModuleData, m_transitionFrames ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}

	UnsignedInt m_transitionFrames;
};

//-------------------------------------------------------------------------------------------------
/** The AnimationSteering Update module */
//-------------------------------------------------------------------------------------------------
class AnimationSteeringUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( AnimationSteeringUpdate, "AnimationSteeringUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( AnimationSteeringUpdate, AnimationSteeringUpdateModuleData );

public:

	AnimationSteeringUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	virtual UpdateSleepTime update( void ); ///< Here's the actual work of Upgrading

protected:

  ModelConditionFlagType m_currentTurnAnim;
	UnsignedInt m_nextTransitionFrame;
};

#endif  // end __ANIMATION_STEERING_UPDATE_H
