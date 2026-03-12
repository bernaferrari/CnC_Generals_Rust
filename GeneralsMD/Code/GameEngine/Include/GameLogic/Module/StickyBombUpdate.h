// FILE: StickyBombUpdate.h ////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, July 2002
// Desc:   Similar to ParachuteContain, this module is used essentially to attach a bomb to an object
//         moving around. The sticky bomb position simply updates to the specified bone until it explodes.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __STICK_BOMB_UPDATE_H
#define __STICK_BOMB_UPDATE_H

#include "GameLogic/Module/UpdateModule.h"

class WeaponTemplate;
class FXList;

//-------------------------------------------------------------------------------------------------
class StickyBombUpdateModuleData : public UpdateModuleData
{
public:
	AsciiString			m_attachToBone;
	Real						m_offsetZ;
	WeaponTemplate*	m_geometryBasedDamageWeaponTemplate;
	FXList*					m_geometryBasedDamageFX;

	StickyBombUpdateModuleData()
	{
		m_offsetZ = 10.0f;
		m_geometryBasedDamageWeaponTemplate = NULL;
		m_geometryBasedDamageFX = NULL;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "AttachToTargetBone",				INI::parseAsciiString,		NULL, offsetof( StickyBombUpdateModuleData, m_attachToBone ) },
			{ "OffsetZ",									INI::parseReal,						NULL, offsetof( StickyBombUpdateModuleData, m_offsetZ ) },
			{ "GeometryBasedDamageWeapon",INI::parseWeaponTemplate, NULL, offsetof( StickyBombUpdateModuleData, m_geometryBasedDamageWeaponTemplate ) },
			{ "GeometryBasedDamageFX",		INI::parseFXList,					NULL, offsetof( StickyBombUpdateModuleData, m_geometryBasedDamageFX ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
class StickyBombUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( StickyBombUpdate, "StickyBombUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( StickyBombUpdate, StickyBombUpdateModuleData )

public:

	StickyBombUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();

	virtual UpdateSleepTime update();							///< called once per frame

	void initStickyBomb( Object *object, const Object *bomber, const Coord3D *specificPos = NULL );
	void detonate();
	Bool isTimedBomb() const { return m_dieFrame > 0; }
	UnsignedInt getDetonationFrame() const { return m_dieFrame; }
	Object* getTargetObject() const;
	void setTargetObject( Object *obj );

private:

	ObjectID			m_targetID;
	UnsignedInt		m_dieFrame;
	UnsignedInt   m_nextPingFrame;
};

#endif // __STICK_BOMB_UPDATE_H

