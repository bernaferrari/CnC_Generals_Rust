// FILE: CommandButtonHuntUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: John Ahlquist, Sept. 2002
// Desc:   Update module to handle wounded idle infantry finding a heal unit or structure.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __COMMAND_BUTTON_HUNT_UPDATE_H_
#define __COMMAND_BUTTON_HUNT_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/KindOf.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ThingTemplate;
class WeaponTemplate;


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class CommandButtonHuntUpdateModuleData : public ModuleData
{
public:
	UnsignedInt			m_scanFrames;
	Real						m_scanRange;

	CommandButtonHuntUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class CommandButtonHuntUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CommandButtonHuntUpdate, "CommandButtonHuntUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CommandButtonHuntUpdate, CommandButtonHuntUpdateModuleData );

public:

	CommandButtonHuntUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();
	virtual UpdateSleepTime update();

	void setCommandButton(const AsciiString& buttonName);

protected:
	Object* scanClosestTarget(void);
	UpdateSleepTime huntSpecialPower(AIUpdateInterface *ai);
	UpdateSleepTime huntWeapon(AIUpdateInterface *ai);
	UpdateSleepTime huntEnter( AIUpdateInterface *ai );


protected:
	AsciiString		m_commandButtonName;
	const CommandButton *m_commandButton;
};


#endif

