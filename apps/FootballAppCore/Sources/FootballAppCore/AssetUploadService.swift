import Foundation

/// Logófeltöltés a helyi API-ba (S185/S8). Csak WebP/PNG, legfeljebb 2 MB;
/// a fájl tartalom-címzett kulccsal az agent asset-tárába kerül, nem a repóba.
public protocol AssetUploadService: Sendable {
    func upload(entityKind: String, entityID: UUID, imageData: Data, filename: String) async throws -> URL
}

public enum AssetUploadError: Error, Equatable, Sendable {
    case tooLarge
    case unsupportedFormat
    case unauthorized
    case invalidEntity
    case server
}

public struct HTTPAssetUploadService: AssetUploadService {
    private let baseURL: URL
    private let opsToken: String
    private let session: URLSession

    public init(baseURL: URL, opsToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.opsToken = opsToken
        self.session = session
    }

    public func upload(
        entityKind: String,
        entityID: UUID,
        imageData: Data,
        filename: String
    ) async throws -> URL {
        guard imageData.count <= 2 * 1024 * 1024 else { throw AssetUploadError.tooLarge }
        let isPNG = imageData.starts(with: [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        let isWebP = imageData.count >= 12
            && String(decoding: imageData[0..<4], as: UTF8.self) == "RIFF"
            && String(decoding: imageData[8..<12], as: UTF8.self) == "WEBP"
        guard isPNG || isWebP else { throw AssetUploadError.unsupportedFormat }

        let boundary = "FootballApp-\(UUID().uuidString)"
        var body = Data()
        func field(_ name: String, _ value: String) {
            body.append(Data("--\(boundary)\r\nContent-Disposition: form-data; name=\"\(name)\"\r\n\r\n\(value)\r\n".utf8))
        }
        field("entity_kind", entityKind)
        field("entity_id", entityID.uuidString.lowercased())
        field("filename", filename)
        body.append(Data("--\(boundary)\r\nContent-Disposition: form-data; name=\"file\"; filename=\"\(filename)\"\r\nContent-Type: application/octet-stream\r\n\r\n".utf8))
        body.append(imageData)
        body.append(Data("\r\n--\(boundary)--\r\n".utf8))

        var request = URLRequest(
            url: baseURL.appendingPathComponent("v1/asset/upload")
        )
        request.httpMethod = "POST"
        request.setValue("Bearer \(opsToken)", forHTTPHeaderField: "Authorization")
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        request.httpBody = body

        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw AssetUploadError.server }
        switch http.statusCode {
        case 200...299:
            guard let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let path = payload["url"] as? String,
                  let url = URL(string: path, relativeTo: baseURL) else {
                throw AssetUploadError.server
            }
            return url
        case 401: throw AssetUploadError.unauthorized
        case 400, 404: throw AssetUploadError.invalidEntity
        default: throw AssetUploadError.server
        }
    }
}
