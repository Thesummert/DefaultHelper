#import <AppKit/AppKit.h>
#import <CoreServices/CoreServices.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

typedef void (*AppCallback)(void *, const char *, const char *, const char *, const unsigned char *, size_t, bool);
typedef void (*DoneCallback)(void *, const char *);

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

void dh_lookup(const char *raw, bool scheme, void *ctx, AppCallback callback) {
    @autoreleasepool {
        NSString *identifier = [NSString stringWithUTF8String:raw];
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

void dh_set(const char *raw, bool scheme, const char *path, const char *bundle, void *ctx, DoneCallback done) {
    @autoreleasepool {
        NSString *identifier = [NSString stringWithUTF8String:raw];
        NSString *appPath = [NSString stringWithUTF8String:path];
        if (@available(macOS 12.0, *)) {
            NSURL *url = appPath.length ? [NSURL fileURLWithPath:appPath] : nil;
            if (!url) { done(ctx, "无法找到应用的位置。"); return; }
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
