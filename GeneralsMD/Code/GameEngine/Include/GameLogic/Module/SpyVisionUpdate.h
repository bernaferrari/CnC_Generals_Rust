// FILE: SpyVisionUpdate.h /////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, September 2002
// Desc:   Special Power will spy on the vision of all enemy players.  
//				Putting a SpecialPower in a behavior takes a big huge amount of code duplication and
//				has no precedent.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SPY_VISION_UPDATE_H
#define _SPY_VISION_UPDATE_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
#include "GameLogic/Module/UpgradeModule.h"

class Player;

//-------------------------------------------------------------------------------------------------
class SpyVisionUpdateModuleData : public UpdateModuleData
{
public:
	UpgradeMuxData	m_upgradeMuxData;

	Bool						m_needsUpgrade;
	Bool						m_selfPowered;
	UnsignedInt			m_selfPoweredDuration;
	UnsignedInt			m_selfPoweredInterval;
	KindOfMaskType	m_spyOnKindof;

	SpyVisionUpdateModuleData()
	{
		m_needsUpgrade = FALSE;
		m_selfPowered = FALSE;
		m_selfPoweredDuration = 0;
		m_selfPoweredInterval = 0;
		m_spyOnKindof = KINDOFMASK_NONE;
		m_spyOnKindof.flip();
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
		static const FieldParse dataFieldParse[] = 
		{
			{ "NeedsUpgrade",					INI::parseBool,									NULL, offsetof( SpyVisionUpdateModuleData, m_needsUpgrade ) },
			{ "SelfPowered",					INI::parseBool,									NULL, offsetof( SpyVisionUpdateModuleData, m_selfPowered ) },
			{ "SelfPoweredDuration",	INI::parseDurationUnsignedInt,	NULL, offsetof( SpyVisionUpdateModuleData, m_selfPoweredDuration ) },
			{ "SelfPoweredInterval",	INI::parseDurationUnsignedInt,	NULL, offsetof( SpyVisionUpdateModuleData, m_selfPoweredInterval ) },
			{ "SpyOnKindof",					KindOfMaskType::parseFromINI,		NULL, offsetof( SpyVisionUpdateModuleData, m_spyOnKindof ) },
			{ 0, 0, 0, 0 }
		};

		UpdateModuleData::buildFieldParse(p);
		p.add(dataFieldParse);
		p.add(UpgradeMuxData::getFieldParse(), offsetof( SpyVisionUpdateModuleData, m_upgradeMuxData ));
	}
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SpyVisionUpdate : public UpdateModule, public UpgradeMux
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SpyVisionUpdate, "SpyVisionUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SpyVisionUpdate, SpyVisionUpdateModuleData )

public:

	SpyVisionUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual SpyVisionUpdate* getSpyVisionUpdate() { return this; }

	// module methods
	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE; }
	virtual void onDelete( void );
	virtual void onCapture( Player *oldOwner, Player *newOwner );
	virtual void onDisabledEdge( Bool nowDisabled );

	// BehaviorModule
	virtual UpgradeModuleInterface* getUpgrade() { return this; }

	//Update module
	virtual UpdateSleepTime update( void );

	void activateSpyVision( UnsignedInt duration );

	void setDisabledUntilFrame( UnsignedInt frame );
	UnsignedInt getDisabledUntilFrame() const { return m_disabledUntilFrame; }

protected:

	// UpgradeMux functions.  Mux standing, of course, for Majorly Ugly Xhitcode
	virtual void upgradeImplementation();
	virtual void getUpgradeActivationMasks(UpgradeMaskType& activation, UpgradeMaskType& conflicting) const
	{
		getSpyVisionUpdateModuleData()->m_upgradeMuxData.getUpgradeActivationMasks(activation, conflicting);
	}
	virtual void performUpgradeFX()
	{
		getSpyVisionUpdateModuleData()->m_upgradeMuxData.performUpgradeFX(getObject());
	}
	virtual void processUpgradeRemoval()
	{
		// I can't take it any more.  Let the record show that I think the UpgradeMux multiple inheritence is CRAP.
		getSpyVisionUpdateModuleData()->m_upgradeMuxData.muxDataProcessUpgradeRemoval(getObject());
	}

	virtual Bool requiresAllActivationUpgrades() const
	{
		return getSpyVisionUpdateModuleData()->m_upgradeMuxData.m_requiresAllTriggers;
	}
	inline Bool isUpgradeActive() const { return isAlreadyUpgraded(); }
	virtual Bool isSubObjectsUpgrade() { return false; }

private:

	void doActivationWork( Player *playerToSetFor, Bool setting );

	UnsignedInt m_deactivateFrame;
	UnsignedInt m_disabledUntilFrame; //sabotaged, emp'd, etc.
	Bool m_currentlyActive;
	Bool m_resetTimersNextUpdate;
};

#endif 

