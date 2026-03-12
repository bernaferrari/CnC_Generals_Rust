// FILE: GrantUpgradeCreate.h //////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, April 2002
// Desc:   GrantUpgrade create module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GRANTUPGRADECREATE_H_
#define __GRANTUPGRADECREATE_H_

#define DEFINE_OBJECT_STATUS_NAMES

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CreateModule.h"
#include "GameLogic/Object.h"
#include "Common/ObjectStatusTypes.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
/** The GrantUpgrade create module */
//-------------------------------------------------------------------------------------------------

class GrantUpgradeCreateModuleData : public CreateModuleData
{
public:
	AsciiString		m_upgradeName;			///< name of the upgrade to be granted.
	ObjectStatusMaskType m_exemptStatus; ///< do not execute if this status is set in the object

	GrantUpgradeCreateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class GrantUpgradeCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( GrantUpgradeCreate, "GrantUpgradeCreate" );
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( GrantUpgradeCreate, GrantUpgradeCreateModuleData );


public:

	GrantUpgradeCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the create method
	virtual void onCreate( void );
	virtual void onBuildComplete();	///< This is called when you are a finished game object

protected:

};

#endif // __GRANTUPGRADECREATE_H_

