// WeaponStatus.h /////////////////////////////////////////////////////////////////////////////////
// Part of header detangling
// JKMCD Aug 2002

#pragma once
#ifndef __WEAPONSTATUS_H__
#define __WEAPONSTATUS_H__

enum WeaponStatus
{
	READY_TO_FIRE,
	OUT_OF_AMMO,
	BETWEEN_FIRING_SHOTS,
	RELOADING_CLIP,
	PRE_ATTACK,

	WEAPON_STATUS_COUNT	// keep last
};

#endif /* __WEAPONSTATUS_H__ */