// WeaponSetType.h ////////////////////////////////////////////////////////////////////////////////
// Part of header detangling
// JKMCD Aug 2002

#pragma once
#ifndef __WEAPONSETTYPE_H__
#define __WEAPONSETTYPE_H__

//-------------------------------------------------------------------------------------------------
// IMPORTANT NOTE: you should endeavor to set up states such that the most "normal"
// state is defined by the bit being off. That is, the typical "normal" condition
// has all condition flags set to zero.
//
// IMPORTANT NOTE #2: if you add or modify this list, be sure to update TheWeaponSetNames, 
// *and* TheWeaponSetTypeToModelConditionTypeMap!
//
enum WeaponSetType
{
	// The access and use of this enum has the bit shifting built in, so this is a 0,1,2,3,4,5 enum
	WEAPONSET_VETERAN		= 0,
	WEAPONSET_ELITE,
	WEAPONSET_HERO,
	WEAPONSET_PLAYER_UPGRADE,			// This weapon set flag comes from a purchased upgrade to the player
	WEAPONSET_CRATEUPGRADE_ONE,
	WEAPONSET_CRATEUPGRADE_TWO,
	WEAPONSET_VEHICLE_HIJACK,
	WEAPONSET_CARBOMB,
	WEAPONSET_MINE_CLEARING_DETAIL,
	WEAPONSET_RIDER1, //Kris: Added these for different combat-bike riders
	WEAPONSET_RIDER2,
	WEAPONSET_RIDER3,
	WEAPONSET_RIDER4,
	WEAPONSET_RIDER5,
	WEAPONSET_RIDER6,
	WEAPONSET_RIDER7,
	WEAPONSET_RIDER8,

	WEAPONSET_COUNT			///< keep last, please
};

#endif /* __WEAPONSETTYPE_H__ */