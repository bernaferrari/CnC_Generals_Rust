// FILE: SpyVisionSpecialPower.h /////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, September 2002
// Desc:   Special Power will spy on the vision of all enemy players.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SPY_VISION_SPECIAL_POWER_H_
#define __SPY_VISION_SPECIAL_POWER_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/SpecialPowerModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class FXList;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SpyVisionSpecialPowerModuleData : public SpecialPowerModuleData
{

public:

	SpyVisionSpecialPowerModuleData( void );

	static void buildFieldParse( MultiIniFieldParse& p );

	UnsignedInt m_baseDurationInFrames;		///< duration of the demoralization (in frames)
	UnsignedInt m_bonusDurationPerCapturedInFrames;	///< additional duration added for each prisoner we have
	UnsignedInt m_maxDurationInFrames;		///< no matter how many prisoners we have, this is max

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SpyVisionSpecialPower : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SpyVisionSpecialPower, "SpyVisionSpecialPower" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SpyVisionSpecialPower, SpyVisionSpecialPowerModuleData )

public:

	SpyVisionSpecialPower( Thing *thing, const ModuleData *moduleData );
	// virtual destructor prototype provided by memory pool object

	virtual void doSpecialPower( UnsignedInt commandOptions );

protected:

};

#endif
