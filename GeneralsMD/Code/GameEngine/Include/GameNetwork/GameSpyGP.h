// FILE: GameSpyGP.h //////////////////////////////////////////////////////
// Generals GameSpy GP (Buddy)
// Author: Matthew D. Campbell, March 2002

#pragma once

#ifndef __GAMESPYGP_H__
#define __GAMESPYGP_H__

#include "GameSpy/GP/GP.h"

void GPRecvBuddyRequestCallback(GPConnection * connection, GPRecvBuddyRequestArg * arg, void * param);
void GPRecvBuddyMessageCallback(GPConnection * pconnection, GPRecvBuddyMessageArg * arg, void * param);
void GPRecvBuddyStatusCallback(GPConnection * connection, GPRecvBuddyStatusArg * arg, void * param);
void GPErrorCallback(GPConnection * pconnection, GPErrorArg * arg, void * param);
void GPConnectCallback(GPConnection * pconnection, GPConnectResponseArg * arg, void * param);
void GameSpyUpdateBuddyOverlay(void);

extern GPConnection *TheGPConnection;

Bool IsGameSpyBuddy(GPProfile id);

#endif // __GAMESPYGP_H__
