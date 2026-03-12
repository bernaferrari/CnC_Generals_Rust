// FILE: FireWeaponCollide.h ///////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood  April 2002
// Desc:   Shoot something that collides with me every frame with my weapon
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FireWeaponCollide_H_
#define __FireWeaponCollide_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/CollideModule.h"
#include "GameLogic/Weapon.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

//-------------------------------------------------------------------------------------------------
class FireWeaponCollideModuleData : public CollideModuleData
{
public:
	const WeaponTemplate* m_collideWeaponTemplate;
	ObjectStatusMaskType m_requiredStatus;
	ObjectStatusMaskType m_forbiddenStatus;
	Bool m_fireOnce;

	FireWeaponCollideModuleData()
	{
		m_collideWeaponTemplate = NULL;
		m_fireOnce = FALSE;
	}

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class FireWeaponCollide : public CollideModule
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FireWeaponCollide, FireWeaponCollideModuleData );
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FireWeaponCollide, "FireWeaponCollide" )

public:

	FireWeaponCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	virtual void onCollide( Object *other, const Coord3D *loc, const Coord3D *normal );

	virtual Bool shouldFireWeapon();

private:
	Weapon* m_collideWeapon;
	Bool m_everFired;

};


#endif

