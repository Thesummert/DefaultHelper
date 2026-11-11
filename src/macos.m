#import <AppKit/AppKit.h>
#import <CoreServices/CoreServices.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

typedef void (*AppCallback)(void *, const char *, const char *, const char *, const unsigned char *, size_t, bool);
typedef void (*DoneCallback)(void *, const char *);

static NSString *type_identifier(NSString *raw, int kind) {
    if (kind != 2) return raw;
    if (@available(macOS 12.0, *)) {
        UTType *type = [UTType typeWithFilenameExtension:raw];
        if (type) return type.identifier;
    }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    return CFBridgingRelease(UTTypeCreatePreferredIdentifierForTag(
        kUTTagClassFilenameExtension, (__bridge CFStringRef)raw, NULL));
#pragma clang diagnostic pop
}

static void emit_app(NSURL *url, NSString *fallback, BOOL current, void *ctx, AppCallback callback) {
    NSBundle *bundle = url ? [NSBundle bundleWithURL:url] : nil;
    NSString *identifier = bundle.bundleIdentifier ?: fallback ?: @"";
    NSString *name = [bundle objectForInfoDictionaryKey:@"CFBundleDisplayName"] ?: [bundle objectForInfoDictionaryKey:@"CFBundleName"] ?: url.lastPathComponent.stringByDeletingPathExtension ?: identifier;
    NSData *png = nil;
    if (url) {
        NSImage *icon = [[NSWorkspace sharedWorkspace] iconForFile:url.path];
        // Keep icon conversion and transfer small even for high-resolution app icons.
        CGImageRef source = [icon CGImageForProposedRect:NULL context:nil hints:nil];
        if (source) {
            CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
            CGContextRef context = CGBitmapContextCreate(NULL, 96, 96, 8, 0, colorSpace, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
            CGColorSpaceRelease(colorSpace);
            if (context) {
                CGContextSetInterpolationQuality(context, kCGInterpolationHigh);
                CGContextDrawImage(context, CGRectMake(0, 0, 96, 96), source);
                CGImageRef scaled = CGBitmapContextCreateImage(context);
                CGContextRelease(context);
                if (scaled) {
                    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:scaled];
                    CGImageRelease(scaled);
                    png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
                }
            }
        }
    }
    callback(ctx, identifier.UTF8String, name.UTF8String, (url.path ?: @"").UTF8String, png.bytes, png.length, current);
}

void dh_lookup(const char *raw, int kind, void *ctx, AppCallback callback) {
    @autoreleasepool {
        NSString *identifier = type_identifier([NSString stringWithUTF8String:raw], kind);
        if (!identifier.length) return;
        BOOL scheme = kind == 1;
        NSWorkspace *workspace = [NSWorkspace sharedWorkspace];
        if (@available(macOS 12.0, *)) {
            NSURL *current = nil; NSArray<NSURL *> *apps = @[];
            if (scheme) {
                NSURLComponents *components = [NSURLComponents new]; components.scheme = identifier; components.host = @"example.com";
                NSURL *url = components.URL;
                if (url) { current = [workspace URLForApplicationToOpenURL:url]; apps = [workspace URLsForApplicationsToOpenURL:url]; }
            } else {
                UTType *type = [UTType typeWithIdentifier:identifier];
                if (type) { current = [workspace URLForApplicationToOpenContentType:type]; apps = [workspace URLsForApplicationsToOpenContentType:type]; }
            }
            if (current) emit_app(current, nil, YES, ctx, callback);
            for (NSURL *url in apps) emit_app(url, nil, NO, ctx, callback);
        } else {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
            CFStringRef current = scheme ? LSCopyDefaultHandlerForURLScheme((__bridge CFStringRef)identifier) : LSCopyDefaultRoleHandlerForContentType((__bridge CFStringRef)identifier, kLSRolesAll);
            CFArrayRef apps = scheme ? LSCopyAllHandlersForURLScheme((__bridge CFStringRef)identifier) : LSCopyAllRoleHandlersForContentType((__bridge CFStringRef)identifier, kLSRolesAll);
#pragma clang diagnostic pop
            if (current) { NSString *bid = CFBridgingRelease(current); emit_app([workspace URLForApplicationWithBundleIdentifier:bid], bid, YES, ctx, callback); }
            for (NSString *bid in CFBridgingRelease(apps)) emit_app([workspace URLForApplicationWithBundleIdentifier:bid], bid, NO, ctx, callback);
        }
    }
}

const char *dh_custom(const char *path, void *ctx, AppCallback callback) {
    @autoreleasepool {
        NSString *appPath = [NSString stringWithUTF8String:path];
        NSURL *url = [NSURL fileURLWithPath:appPath];
        NSBundle *bundle = [NSBundle bundleWithURL:url];
        if (![appPath.pathExtension.lowercaseString isEqualToString:@"app"]
                || !bundle.bundleIdentifier.length || !bundle.executablePath.length
                || ![[NSFileManager defaultManager] isExecutableFileAtPath:bundle.executablePath])
            return "请选择有效的 macOS .app 应用包。";
        emit_app(url, nil, NO, ctx, callback);
        return NULL;
    }
}

void dh_set(const char *raw, int kind, const char *path, const char *bundle, void *ctx, DoneCallback done) {
    @autoreleasepool {
        NSString *identifier = type_identifier([NSString stringWithUTF8String:raw], kind);
        if (!identifier.length) { done(ctx, "无法识别该文件类型或扩展名。"); return; }
        BOOL scheme = kind == 1;
        NSString *appPath = [NSString stringWithUTF8String:path];
        NSURL *appURL = appPath.length ? [NSURL fileURLWithPath:appPath] : nil;
        if (!appURL || ![[NSFileManager defaultManager] fileExistsAtPath:appPath]) {
            done(ctx, "应用已被移除，请重新添加或查询。"); return;
        }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        OSStatus registration = LSRegisterURL((__bridge CFURLRef)appURL, true);
#pragma clang diagnostic pop
        if (registration) { done(ctx, "无法向系统注册此应用。"); return; }
        if (@available(macOS 12.0, *)) {
            NSURL *url = appURL;
            void (^completion)(NSError *) = ^(NSError *error) { done(ctx, error ? error.localizedDescription.UTF8String : NULL); };
            if (scheme) [[NSWorkspace sharedWorkspace] setDefaultApplicationAtURL:url toOpenURLsWithScheme:identifier completionHandler:completion];
            else {
                UTType *type = [UTType typeWithIdentifier:identifier];
                if (!type) { done(ctx, "无法识别该文件类型。往系统中注册此类型后重试。"); return; }
                [[NSWorkspace sharedWorkspace] setDefaultApplicationAtURL:url toOpenContentType:type completionHandler:completion];
            }
        } else {
            NSString *bid = [NSString stringWithUTF8String:bundle];
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
            OSStatus status = scheme ? LSSetDefaultHandlerForURLScheme((__bridge CFStringRef)identifier, (__bridge CFStringRef)bid) : LSSetDefaultRoleHandlerForContentType((__bridge CFStringRef)identifier, kLSRolesAll, (__bridge CFStringRef)bid);
#pragma clang diagnostic pop
            if (status) done(ctx, [NSString stringWithFormat:@"设置失败（macOS 错误代码 %d）。", (int)status].UTF8String); else done(ctx, NULL);
        }
    }
}
