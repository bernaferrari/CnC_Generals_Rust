// FILE: LockWeaponCreate.h //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2003
// Desc:   Locks the weapon choice to the slot specified on creation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __LOCKWEAPONCREATE_H_
#define __LOCKWEAPONCREATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CreateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
/** The GrantUpgrade create module */
//-------------------------------------------------------------------------------------------------

class LockWeaponCreateModuleData : public CreateModuleData
{
public:
	WeaponSlotType m_slotToLock; ///< slot to lock

	LockWeaponCreateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class LockWeaponCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( LockWeaponCreate, "LockWeaponCreate" );
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( LockWeaponCreate, LockWeaponCreateModuleData );


public:

	LockWeaponCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the create method
	virtual void onCreate( void );
	virtual void onBuildComplete();	///< This is called when you are a finished game object

protected:

};

#endif

