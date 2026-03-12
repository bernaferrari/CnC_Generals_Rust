// FILE: JetSlowDeathBehavior.h ///////////////////////////////////////////////////////////////////
// Author: Colin Day
// Desc:   Death sequence for jets
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __JET_SLOW_DEATH_BEHAVIOR_H_
#define __JET_SLOW_DEATH_BEHAVIOR_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "Common/AudioEventRTS.h"
#include "GameLogic/Module/BehaviorModule.h"
#include "GameLogic/Module/SlowDeathBehavior.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class FXList;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class JetSlowDeathBehaviorModuleData : public SlowDeathBehaviorModuleData
{

public:

	JetSlowDeathBehaviorModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	const FXList *m_fxOnGroundDeath;							///< fx list executed on death when destoyed on ground
	const ObjectCreationList *m_oclOnGroundDeath;	///< ocl list executed on death when destroyed on ground

	const FXList *m_fxInitialDeath;								///< FXList for initial death
	const ObjectCreationList *m_oclInitialDeath;	///< OCL for initial death

	UnsignedInt m_delaySecondaryFromInitialDeath;	///< delay (in frames) from initial death, to the secondary event
	const FXList *m_fxSecondary;									///< FXList for secondary event
	const ObjectCreationList *m_oclSecondary;			///< OCL for secondary event

	const FXList *m_fxHitGround;									///< FXList for hit ground
	const ObjectCreationList *m_oclHitGround;			///< OCL for hit ground
	
	UnsignedInt m_delayFinalBlowUpFromHitGround;	///< delay (in frames) from hit ground, to final explosion
	const FXList *m_fxFinalBlowUp;								///< FxList for final blow up
	const ObjectCreationList *m_oclFinalBlowUp;		///< OCL for final blow up

	Real m_rollRate;															///< our initial roll rate
	Real m_rollRateDelta;													///< how our roll rate changes over time
	Real m_pitchRate;															///< spin speed on another axis after hitting the ground
	Real m_fallHowFast;														///< a fraction of gravity we use to modify the jet locmotor lift

	AudioEventRTS m_deathLoopSound;								///< looping death sound

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class JetSlowDeathBehavior : public SlowDeathBehavior
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( JetSlowDeathBehavior, "JetSlowDeathBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( JetSlowDeathBehavior, JetSlowDeathBehaviorModuleData )

public:

	JetSlowDeathBehavior( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	// slow death methods
	virtual void onDie( const DamageInfo *damageInfo );
	virtual void beginSlowDeath( const DamageInfo *damageInfo );
	virtual UpdateSleepTime update( void );

protected:

	UnsignedInt m_timerDeathFrame;					///< fame we died on
	UnsignedInt m_timerOnGroundFrame;				///< frame we landed on the ground on

	Real m_rollRate;												///< our roll rate

	AudioEventRTS m_deathLoopSound;					///< death loop sound

};


#endif  // end __JET_SLOW_DEATH_BEHAVIOR_H_
