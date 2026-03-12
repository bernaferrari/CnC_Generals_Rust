// FILE: MoneyCrateCollide.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that gives x money to the collider
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef MONEY_CRATE_COLLIDE_H_
#define MONEY_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"
#include "GameLogic/Module/AutoDepositUpdate.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class MoneyCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_moneyProvided;
	std::list<upgradePair> m_upgradeBoost;

	MoneyCrateCollideModuleData()
	{
		m_moneyProvided = 0;
		m_upgradeBoost.clear();
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "MoneyProvided",	INI::parseUnsignedInt,	NULL, offsetof( MoneyCrateCollideModuleData, m_moneyProvided ) },
			{ "UpgradedBoost",	parseUpgradePair,		NULL, offsetof( MoneyCrateCollideModuleData, m_upgradeBoost ) },

			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}
};

//-------------------------------------------------------------------------------------------------
class MoneyCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( MoneyCrateCollide, "MoneyCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( MoneyCrateCollide, MoneyCrateCollideModuleData );

public:

	MoneyCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	Int getUpgradedSupplyBoost( Object *other ) const;

};

#endif
