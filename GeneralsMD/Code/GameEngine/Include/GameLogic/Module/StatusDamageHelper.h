// FILE: StatusDamageHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Object helper - Clears Status conditions on a timer.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __StatusDamageHelper_H_
#define __StatusDamageHelper_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ObjectHelper.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class StatusDamageHelperModuleData : public ModuleData
{

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class StatusDamageHelper : public ObjectHelper
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( StatusDamageHelper, StatusDamageHelperModuleData )
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(StatusDamageHelper, "StatusDamageHelper" )	

public:

	StatusDamageHelper( Thing *thing, const ModuleData *modData );
	// virtual destructor prototype provided by memory pool object

	virtual DisabledMaskType getDisabledTypesToProcess() const { return DISABLEDMASK_ALL; }
	virtual UpdateSleepTime update();

	void doStatusDamage( ObjectStatusTypes status, Real duration );

protected:
	ObjectStatusTypes m_statusToHeal;
	UnsignedInt m_frameToHeal;
	void clearStatusCondition();
};


#endif  // end __StatusDamageHelper_H_
