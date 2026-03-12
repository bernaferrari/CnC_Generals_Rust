// FILE: UpgradeDie.h /////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, August 2002
// Desc:   Free's producer's upgrade (assuming this object was created via an upgrade).
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __UPGRADEDIE_H
#define __UPGRADEDIE_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/INI.h"
#include "GameLogic/Module/DieModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class UpgradeDieModuleData : public DieModuleData
{
public:
	AsciiString m_upgradeName;

	UpgradeDieModuleData(){}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    
		DieModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "UpgradeToRemove",					INI::parseAsciiString,		NULL, offsetof( UpgradeDieModuleData, m_upgradeName ) },
			{ 0, 0, 0, 0 }
		};

    p.add(dataFieldParse);
	}
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class UpgradeDie : public DieModule
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( UpgradeDie, "UpgradeDie" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( UpgradeDie, UpgradeDieModuleData )

public:

	UpgradeDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onDie( const DamageInfo *damageInfo ); 

};

#endif // __UPGRADEDIE_H

