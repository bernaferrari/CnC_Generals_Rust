// FILE: WeaponBonusUpdate.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002-2003 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	July 2003
//
//	Filename: 	WeaponBonusUpdate.cpp
//
//	author:		Graham Smallwood
//	
//	purpose:	Like healing in that it can affect just me or people around, 
//						except this gives a Weapon Bonus instead of health
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __WEAPON_BONUS_UPDATE_H_
#define __WEAPON_BONUS_UPDATE_H_

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "GameLogic/Module/UpdateModule.h"
//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
enum WeaponBonusConditionType;

//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class WeaponBonusUpdateModuleData : public UpdateModuleData
{
public:

	WeaponBonusUpdateModuleData();

	KindOfMaskType						m_requiredAffectKindOf;						///< Must be set on target
	KindOfMaskType						m_forbiddenAffectKindOf;	///< Must be clear on target
	UnsignedInt								m_bonusDuration;					///< How long a hit lasts on target
	UnsignedInt								m_bonusDelay;							///< How often to pulse
	Real											m_bonusRange;							///< How far to affect
	WeaponBonusConditionType	m_bonusConditionType;			///< Status to give

	static void buildFieldParse(MultiIniFieldParse& p);
};


//-------------------------------------------------------------------------------------------------
class WeaponBonusUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( WeaponBonusUpdate, "WeaponBonusUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( WeaponBonusUpdate, WeaponBonusUpdateModuleData )

public:

	WeaponBonusUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update( void );

protected:

};


//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

#endif
