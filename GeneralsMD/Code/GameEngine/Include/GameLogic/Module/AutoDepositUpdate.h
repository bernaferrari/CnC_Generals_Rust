// FILE: AutoDepositUpdate.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Aug 2002
//
//	Filename: 	AutoDepositUpdate.h
//
//	author:		Chris Huybregts
//	
//	purpose:	Auto Deposit Update Module
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __AUTO_DEPOSIT_UPDATE_H_
#define __AUTO_DEPOSIT_UPDATE_H_

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
class Player;
class Thing;
void parseUpgradePair( INI *ini, void *instance, void *store, const void *userData );
struct upgradePair
{
	std::string type;
	Int         amount;
};

//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class AutoDepositUpdateModuleData : public UpdateModuleData
{
public:

	UnsignedInt m_depositFrame;
	Int m_depositAmount;
	Int m_initialCaptureBonus;
	Bool m_isActualMoney;
	std::list<upgradePair> m_upgradeBoost;

	AutoDepositUpdateModuleData()
	{
		m_depositFrame = 0;
		m_depositAmount = 0;
		m_initialCaptureBonus = 0;
		m_isActualMoney = TRUE;
		m_upgradeBoost.clear();
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "DepositTiming",					INI::parseDurationUnsignedInt,		NULL, offsetof( AutoDepositUpdateModuleData, m_depositFrame ) },
			{ "DepositAmount",					INI::parseInt,		NULL, offsetof( AutoDepositUpdateModuleData, m_depositAmount ) },
			{ "InitialCaptureBonus",		INI::parseInt,		NULL, offsetof( AutoDepositUpdateModuleData, m_initialCaptureBonus ) },
			{ "ActualMoney",						INI::parseBool,		NULL, offsetof( AutoDepositUpdateModuleData, m_isActualMoney ) },
			{ "UpgradedBoost",					parseUpgradePair,		NULL, offsetof( AutoDepositUpdateModuleData, m_upgradeBoost ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};


//-------------------------------------------------------------------------------------------------
class AutoDepositUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( AutoDepositUpdate, "AutoDepositUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( AutoDepositUpdate, AutoDepositUpdateModuleData )

public:

	AutoDepositUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void awardInitialCaptureBonus( Player *player );	// Test and award the initial capture bonus
	virtual UpdateSleepTime update( void );

protected:

	Int getUpgradedSupplyBoost() const;

	UnsignedInt m_depositOnFrame;
	Bool m_awardInitialCaptureBonus;
	Bool m_initialized;

};


//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

#endif // __AUTO_DEPOSIT_UPDATE_H_
