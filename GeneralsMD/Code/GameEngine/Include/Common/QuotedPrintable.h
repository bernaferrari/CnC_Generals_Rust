// FILE: QuotedPrintable.h /////////////////////////////////////////////////////////
// Author: Matt Campbell, February 2002
// Description: Quoted-printable encode/decode
////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __QUOTEDPRINTABLE_H__
#define __QUOTEDPRINTABLE_H__

UnicodeString QuotedPrintableToUnicodeString(AsciiString original);
AsciiString UnicodeStringToQuotedPrintable(UnicodeString original);

AsciiString QuotedPrintableToAsciiString(AsciiString original);
AsciiString AsciiStringToQuotedPrintable(AsciiString original);

#endif // __QUOTEDPRINTABLE_H__