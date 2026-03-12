///////////////////////////////////////////////////////////////////////////////////////////////////
//
// FILE: HijackerUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, July 2002
// Desc:   Allows hijacker to keep with his hijacked vehicle (though hidden) until it dies, then 
// to become a hijacker once more
//
/////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once
 
#ifndef __HIJACKER_UPDATE_H
#define __HIJACKER_UPDATE_H

#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
class HijackerUpdateModuleData : public UpdateModuleData
{
public:
	AsciiString m_attachToBone;
	AsciiString m_parachuteName;

	//StickBombUpdateModuleData();

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "AttachToTargetBone",	INI::parseAsciiString,		NULL, offsetof( HijackerUpdateModuleData, m_attachToBone ) },
			{ "ParachuteName",	INI::parseAsciiString,		NULL, offsetof( HijackerUpdateModuleData, m_parachuteName ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
class HijackerUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( HijackerUpdate, "HijackerUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( HijackerUpdate, HijackerUpdateModuleData )

public:

	HijackerUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();							///< called once per frame

	void setTargetObject( const Object *object );
	Object* getTargetObject() const;
	void setUpdate(Bool u ) {m_update = u;} 
	void setIsInVehicle(Bool i ) {m_isInVehicle = i;} 

private:

	ObjectID m_targetID;
	Coord3D	 m_ejectPos;
	Bool     m_update;
	Bool		 m_isInVehicle;
	Bool		 m_wasTargetAirborne;

//	DieModuleInterface *m_ejectPilotDMI; // point to ejectpilotdiemodule 
																			 // of target vehicle if it has one

};

#endif // __HIJACKER_UPDATE_H

