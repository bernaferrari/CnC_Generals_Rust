// FILE: TempWeaponBonusHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:   Object helper - Clears Temporary weapon bonus effects
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __TempWeaponBonusHelper_H_
#define __TempWeaponBonusHelper_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ObjectHelper.h"

enum WeaponBonusConditionType;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class TempWeaponBonusHelperModuleData : public ModuleData
{

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class TempWeaponBonusHelper : public ObjectHelper
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( TempWeaponBonusHelper, TempWeaponBonusHelperModuleData )
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(TempWeaponBonusHelper, "TempWeaponBonusHelper" )	

public:

	TempWeaponBonusHelper( Thing *thing, const ModuleData *modData );
	// virtual destructor prototype provided by memory pool object

	virtual DisabledMaskType getDisabledTypesToProcess() const { return DISABLEDMASK_ALL; }
	virtual UpdateSleepTime update();

	void doTempWeaponBonus( WeaponBonusConditionType status, UnsignedInt duration );

protected:
	WeaponBonusConditionType m_currentBonus;
	UnsignedInt m_frameToRemove;
	void clearTempWeaponBonus();
};


#endif  // end __TempWeaponBonusHelper_H_
