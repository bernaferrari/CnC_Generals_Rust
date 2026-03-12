#pragma once
#ifndef __NETWORKUTIL_H
#define __NETWORKUTIL_H

#include "GameNetwork/NetworkDefs.h"
#include "GameNetwork/NetworkInterface.h"

UnsignedInt ResolveIP(AsciiString host);
UnsignedShort GenerateNextCommandID();
Bool DoesCommandRequireACommandID(NetCommandType type);
Bool CommandRequiresAck(NetCommandMsg *msg);
Bool CommandRequiresDirectSend(NetCommandMsg *msg);
Bool IsCommandSynchronized(NetCommandType type);
AsciiString GetAsciiNetCommandType(NetCommandType type);

#ifdef DEBUG_LOGGING
extern "C" {
void dumpBufferToLog(const void *vBuf, Int len, const char *fname, Int line);
};
#define LOGBUFFER(buf, len) dumpBufferToLog(buf, len, __FILE__, __LINE__)
#else
#define LOGBUFFER(buf, len) {}
#endif // DEBUG_LOGGING

#endif
