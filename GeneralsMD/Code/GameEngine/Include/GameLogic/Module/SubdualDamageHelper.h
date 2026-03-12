// FILE: SubdualDamageHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Object helper - Clears subdual status and heals subdual damage since Body modules can't have Updates
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SubdualDamageHelper_H_
#define __SubdualDamageHelper_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ObjectHelper.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class SubdualDamageHelperModuleData : public ModuleData
{

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class SubdualDamageHelper : public ObjectHelper
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SubdualDamageHelper, SubdualDamageHelperModuleData )
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(SubdualDamageHelper, "SubdualDamageHelper" )	

public:

	SubdualDamageHelper( Thing *thing, const ModuleData *modData );
	// virtual destructor prototype provided by memory pool object

	virtual DisabledMaskType getDisabledTypesToProcess() const { return DISABLEDMASK_ALL; }
	virtual UpdateSleepTime update();

	void notifySubdualDamage( Real amount );

protected:
	UnsignedInt m_healingStepCountdown;
};


#endif  // end __SubdualDamageHelper_H_
